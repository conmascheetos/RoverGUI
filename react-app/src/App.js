import React, { useState, useEffect } from 'react';
import './App.css';

//filepath for testing (DELETE LATER): ../../../GitHub/Automomous/examples/ARTrackerTest/videos
function App() {
  const [fpsSlider, setFpsSlider] = useState(50); // Initial fps slider value
  const [resolutionSlider, setResolutionSlider] = useState(50); // Initial resolution slider value

  //Need to create a selection of camera names to choose from, and then pass that camera name
  //to the image source to get the video feed from the server

  /*
    human_name: string,
    description: string,
    misc: string,
    index: number
  */
  const [cameras, setCameras] = useState([]) //getting available devices from server

  /*//Effect to get the camera names from the server
  useEffect(() => {
    fetch('/video_feed/available_devices')
      .then(response => response.json())
      .then(data => {
        updateCameraNames(data)
      })
  }, [])*/

  // Fetch Camera(s) Information from Server
  useEffect(() => {
    (async () => {
      let response = await fetch("/stream/available_cameras");
      let cameras = await response.json();

      // Move camera.index.Index to camera.index
      cameras.filter((camera) => {
        // Only keep cameras that have an index (non-ip cameras).
        if (!camera.index.hasOwnProperty("Index"))
          return false;

        camera.index = camera.index.Index;
        return true;
      });
      
      setCameras(cameras);
    })();
  }, []);

  // //test data for camera names
  // useEffect(() => {
  //   setCameras([null, 'Camera 1', 'Camera 2', 'Camera 3'])
  // }, [])

  //On change of the camera selection, add components for camera feed and sliders
  //to control the camera feed
  const [selectedCamera, setSelectedCamera] = useState(null);

  const handleCameraChange = (event) => {
    console.log(JSON.stringify(event.target.value));
    setSelectedCamera(event.target.value);
  };

  return (
    <div className="App">
      <div className="camera-select">
        <label>Select Camera: </label>
        <select onChange={handleCameraChange}>
          {cameras.map((camera, index) => {
            return <option key={index} value={camera.index}>{camera.human_name}</option>
          })}
        </select>
        {selectedCamera && (
        <div>
          <div className="camera-feed">
            <img src={`/video_feed/${selectedCamera}`} alt="Camera Frame" width="600" height="400" />
          </div>
          <div className="slider-container">
            <label htmlFor="fpsSlider">FPS:</label>
            <input
              id="fpsSlider"
              type="range"
              min="0"
              max="100"
              value={fpsSlider}
              onChange={(event) => setFpsSlider(event.target.value)}
            />
            <label htmlFor="resolutionSlider">Resolution:</label>
            <input
              id="resolutionSlider"
              type="range"
              min="0"
              max="100"
              value={resolutionSlider}
              onChange={(event) => setResolutionSlider(event.target.value)}
            />
          </div>
        </div>
      )}
      </div>
    </div>
  );
}

export default App;