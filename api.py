from fastapi import FastAPI
from fastapi.responses import StreamingResponse, HTMLResponse
import cv2
import time
from camera_manager import CameraManger

# Create api
app = FastAPI()

# Create camera manager
camera_manager = CameraManger()

@app.get("/")
async def root():
    """
    Just for testing
    """
    return HTMLResponse("""
    <html>
        <h1>Help</h1>
    </html>
    """, status_code=200)

def fps_to_ms(fps: int) -> float:
    """
    Convert frames per second to frames per miliseconds (time between each frame)
    """
    return (1/fps) * 1000   

def generate_frames(camera_name: str):
    """
    A generator that will yield the frames for a given camera
    """
    cap = camera_manager.start_video_capture(camera_name)
    cap.set(cv2.CAP_PROP_BUFFERSIZE, 1) # Set camera parameters.
    # Capture video frames at specified frame rate
    time_since_last_frame = 0
    while camera_manager.camera_is_running(camera_name):
        # Get the wait time based on fps
        wait_time = fps_to_ms(camera_manager.get_camrea_fps(camera_name))
        
        # Capture a video frame
        success, frame = cap.read()
        if not success:
            break
        
        # Check if it's time to send a frame
        if time.time() * 1000 - time_since_last_frame > wait_time:
            ret, buffer = cv2.imencode('.jpg', frame)
            frame = buffer.tobytes()
            yield (b'--frame\r\n'
                    b'Content-Type: image/jpeg\r\n\r\n' + frame + b'\r\n')
            time_since_last_frame = time.time() * 1000
        

@app.get("/stream/start/{camera_name}")
async def start_stream(camera_name: str):
    """
    Take in a camera name and start a video stream
    """
    return StreamingResponse(generate_frames(camera_name), media_type="multipart/x-mixed-replace; boundary=frame")

@app.get("/stream/end/{camera_name}")
async def end_stream(camera_name: str):
    camera_manager.end_video_capture(camera_name)
    return True

@app.get('/stream/available_cameras')
async def get_available_cameras():
    available_cameras = camera_manager.get_available_cameras()
    return available_cameras

@app.post('/stream/fps/{camera_name}')
async def set_camera_fps(camera_name: str, fps: int):
    camera_manager.set_camera_fps(camera_name, fps)
    return True
