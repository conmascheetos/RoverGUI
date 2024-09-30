use std::{error::Error, path::Path, sync::{atomic::{AtomicBool, Ordering}, Arc}, thread::{self}, time::Duration};

use rocket::tokio::{sync::{mpsc::{self, Receiver, Sender}, Mutex, Notify}, task, time};
use v4l::{video::{Capture, Output}, FourCC};
use webrtc::{api::{interceptor_registry, media_engine::{MediaEngine, MIME_TYPE_H264}, APIBuilder, API}, ice_transport::ice_connection_state::RTCIceConnectionState, interceptor::registry::Registry, media::Sample, peer_connection::{configuration::RTCConfiguration, peer_connection_state::RTCPeerConnectionState, sdp::session_description::RTCSessionDescription}, rtp_transceiver::rtp_codec::RTCRtpCodecCapability, track::track_local::{track_local_static_sample::TrackLocalStaticSample, TrackLocal}};

pub struct WebcamManager {
    camera_handles: Arc<Mutex<Vec<CameraThreadHandle>>>,
    rtc_api: API
}

impl WebcamManager {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let mut media_engine = MediaEngine::default();
        media_engine.register_default_codecs()?;

        let mut registry = Registry::new();
        registry = interceptor_registry::register_default_interceptors(registry, &mut media_engine)?;

        let rtc_api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        Ok(Self {
            camera_handles: Arc::new(Mutex::new(Vec::new())),
            rtc_api
        })
    }

    pub async fn add_client(&self, camera_path: String, rtc_offer: RTCSessionDescription) -> Result<RTCSessionDescription, Box<dyn Error>> {
        let peer_connection = self.rtc_api.new_peer_connection(RTCConfiguration::default()).await?;
        let peer_connection = Arc::new(peer_connection);
        let peer_disconnected = Arc::new(AtomicBool::new(false));
        let notify_tx = Arc::new(Notify::new());
        let notify_rx = notify_tx.clone();

        let video_track = Arc::new(TrackLocalStaticSample::new(
            RTCRtpCodecCapability {
                mime_type: MIME_TYPE_H264.to_owned(),
                ..Default::default()
            },
            "video".to_owned(),
            "webrtc".to_owned(),
        ));

        let rtp_sender = peer_connection
            .add_track(Arc::clone(&video_track) as Arc<dyn TrackLocal + Send + Sync>)
            .await?;

        // Should drop if connection is canceled
        task::spawn(async move {
            let mut rtcp_buf = vec![0u8; 1500];
            while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
        });
    
        let c_camera_handles = self.camera_handles.clone();
        let c_peer_disconnected = peer_disconnected.clone();
        task::spawn(async move {
            if time::timeout(Duration::from_millis(5000), notify_rx.notified()).await.is_err() {
                return;
            }

            let mut rx = {
                let camera_handles = &mut *c_camera_handles.lock().await;
                match camera_handles.iter().find(|handle| handle.camera_path == camera_path) {
                    Some(handle) => {
                        handle.enroll_rx().await
                    },
                    None => {
                        let handle = CameraThreadHandle::start_camera_thread(&camera_path, c_camera_handles.clone());
                        let rx = handle.enroll_rx().await;
                        
                        camera_handles.push(handle);
                        rx
                    },
                }
            };

            while let Some(bytes) = rx.recv().await {
                if c_peer_disconnected.load(Ordering::SeqCst) {
                    return;
                }

                video_track
                    .write_sample(&Sample {
                        data: bytes.into(),
                        duration: Duration::from_millis(1000),
                        ..Default::default()
                    })
                .await.unwrap();
            }
        });

        peer_connection.on_ice_connection_state_change(Box::new(
            move |connection_state: RTCIceConnectionState| {
                match connection_state {
                    RTCIceConnectionState::Connected => {
                        notify_tx.notify_waiters();
                    },
                    _ => {}
                }
                Box::pin(async {  })
            },
        ));

        let c_peer_connection = peer_connection.clone();
        peer_connection.on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
            match state {
                RTCPeerConnectionState::Disconnected => {
                    let _ = c_peer_connection.close();
                    peer_disconnected.store(true, Ordering::SeqCst);
                },
                _ => {}
            }

            Box::pin(async {})
        }));

        peer_connection.set_remote_description(rtc_offer).await?;
        let answer = peer_connection.create_answer(None).await?;
        let mut ice_gather_rx = peer_connection.gathering_complete_promise().await;
        peer_connection.set_local_description(answer).await?;
        ice_gather_rx.recv().await;

        Ok(peer_connection.local_description().await.ok_or("Failed to Generate Description")?)
    }
}

struct CameraThreadHandle {
    camera_path: String,
    manual_shutdown_needed: Arc<AtomicBool>,
    sink_flush_needed: Arc<AtomicBool>,
    tx_sink: Arc<Mutex<Vec<Sender<Vec<u8>>>>>
}

impl CameraThreadHandle {
    fn start_camera_thread(camera_path: &str, camera_handles: Arc<Mutex<Vec<CameraThreadHandle>>>) -> Self {
        let camera_path = camera_path.to_owned();
        let manual_shutdown_needed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let sink_flush_needed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
        let tx_sink: Arc<Mutex<Vec<Sender<Vec<u8>>>>> = Arc::new(Mutex::new(Vec::new()));

        let c_camera_path = camera_path.clone();
        let c_manual_shutdown_needed = manual_shutdown_needed.clone();
        let c_sink_flush_needed = sink_flush_needed.clone();
        let c_tx_sink = tx_sink.clone();
        thread::spawn(move || {
            let device_path = Path::new(&c_camera_path);
            let max_fps = 30;
        
            let mut device = h264_webcam_stream::get_device(&device_path).unwrap();

            // // Used for deriving actual usable resolutions and frame rates.
            // println!("{:?}", Capture::enum_framesizes(&device, FourCC::new(b"MJPG")).unwrap());
            // println!("{:?}", Capture::enum_frameintervals(&device, FourCC::new(b"MJPG"), 1280, 720).unwrap());
            
            let mut stream = h264_webcam_stream::stream(&mut device, max_fps).unwrap();

            let mut rtc_txs: Vec<Sender<Vec<u8>>> = Vec::new();

            loop {
                if c_manual_shutdown_needed.load(Ordering::SeqCst) {
                    return;
                }

                if c_sink_flush_needed.load(Ordering::SeqCst) {
                    let tx_sink = &mut *c_tx_sink.blocking_lock();

                    while let Some(tx) = tx_sink.pop() {
                        rtc_txs.push(tx);
                    }

                    c_manual_shutdown_needed.store(false, Ordering::SeqCst);
                }

                let (bytes, _) = stream.next(false).unwrap();
                
                rtc_txs.retain(|tx| {
                    tx.blocking_send(bytes.clone()).is_ok()
                });

                if rtc_txs.len() == 0 {
                    let camera_handles = &mut *camera_handles.blocking_lock();
                    camera_handles.retain(|handle| handle.camera_path != c_camera_path);

                    return;
                }
            }
        });

        CameraThreadHandle {
            camera_path,
            manual_shutdown_needed,
            sink_flush_needed,
            tx_sink
        }
    }

    async fn enroll_rx(&self) -> Receiver<Vec<u8>> {
        let (tx, rx) = mpsc::channel::<Vec<u8>>(1);
        let tx_sink = &mut *self.tx_sink.lock().await;

        tx_sink.push(tx);
        self.sink_flush_needed.store(true, Ordering::SeqCst);

        rx
    }
}