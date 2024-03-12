from flask import Flask, Response, render_template
from threading import Thread, Lock
import time
import os


app = Flask(__name__)

# Global variables to hold the current frame, lock for thread-safe access, and the start segment
current_frame = None
frame_lock = Lock()
current_segment = None
current_frame_number = 0

def find_middle_segment():
    segments = sorted([int(name) for name in os.listdir('frames') if os.path.isdir(os.path.join('frames', name)) and name.isdigit()])
    if not segments:
        return None  # Handle the case where no segments are found
    middle_index = len(segments) // 2
    return segments[middle_index]

def update_current_frame():
    global current_frame, current_segment, current_frame_number
    if current_segment is None:
        current_segment = find_middle_segment()

    while True:
        frame_path = f'frames/{current_segment}/{current_frame_number}.jpg'
        if os.path.exists(frame_path):
            with open(frame_path, 'rb') as f:
                with frame_lock:
                    current_frame = f.read()
        
        # Update the frame number and segment
        current_frame_number += 1
        if current_frame_number >= 180:  # Assuming 180 frames per segment
            current_frame_number = 0
            current_segment += 1  # Move to the next segment
            
            # Implement logic to loop or move to the next available segment
            if not os.path.exists(f'frames/{current_segment}'):
                current_segment = find_middle_segment()  # Loop back or find the next segment

        time.sleep(1/30)

@app.route('/')
def index():
    return render_template('index.html')

@app.route('/video_feed')
def video_feed():
    def generate():
        while True:
            with frame_lock:
                frame_data = current_frame
            if frame_data:
                yield (b'--frame\r\n'
                       b'Content-Type: image/jpeg\r\n\r\n' + frame_data + b'\r\n')
            else:
                time.sleep(1/30)  # If no frame is available, wait

    return Response(generate(), mimetype='multipart/x-mixed-replace; boundary=frame')

if __name__ == '__main__':
    # Start the frame updater in a background thread
    thread = Thread(target=update_current_frame)
    thread.daemon = True
    thread.start()

    # Start the Flask app
    app.run(host='0.0.0.0', port=5000, threaded=True)
