<<<<<<< HEAD
// Old WebSocket/WebRTC presets removed - no longer applicable for VNC mode
// VNC configuration is simplified and doesn't require presets
export const presets = {};
=======
export const presets = {
  gaming: {
    bitrate: 12000,
    fps: 60
  },
  desktop: {
    bitrate: 6000,
    fps: 30
  },
  lowBandwidth: {
    bitrate: 2000,
    fps: 15
  }
};
>>>>>>> feat/rdengine-vp9-streaming-with-https
