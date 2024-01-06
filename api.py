import uvicorn
import time
import pytz
import datetime
import os
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse, HTMLResponse
from fastapi.templating import Jinja2Templates
import os

class FrameFetcher:
    def __init__(self):
        self.frame = 0
        self.segment = 0
        
    def _get_current_segment(self):
        # Get and sort segment directories
        segments = [d for d in os.listdir('segments')]
        segments.sort(reverse=True)
        self.segment = int(segments[1])
        return True

    def _check_if_frame_exists(self):
        return os.path.isfile(f"frames/{self.segment}/{self.frame}.jpg")

    def get_next_frame(self):
        if self.frame == max([int(x.split('.')[0]) for x in os.listdir(f'frames/{self.segment}')]):
            self.segment += 1
            self.frame = 0
        else:
            self.frame += 1
        
        return True

frame_fetcher = FrameFetcher()

app = FastAPI()
templates = Jinja2Templates(directory="templates")

@app.get("/stream")
def stream_frames():
    frame_interval = 1.0 / 33.33 # Approximately 33.33 milliseconds for 30 FPS
    frame_fetcher._get_current_segment()
    def generate():
        while True:
            start_time = time.time()
            frame_fetcher.get_next_frame()
            print(f"Segment: {frame_fetcher.segment} Frame: {frame_fetcher.frame}")
            if frame_fetcher._check_if_frame_exists():
                with open(f'frames/{frame_fetcher.segment}/{frame_fetcher.frame}.jpg', "rb") as f:
                    yield (b'--frame\r\n'
                           b'Content-Type: image/jpeg\r\n\r\n' + f.read() + b'\r\n')

            elapsed_time = time.time() - start_time
            wait_time = max(frame_interval - elapsed_time, 0)
            print(f"Segment: {frame_fetcher.segment} Frame: {frame_fetcher.frame} Time: {wait_time}")
            
            time.sleep(.03)

    return StreamingResponse(generate(), media_type="multipart/x-mixed-replace; boundary=frame")

@app.get("/", response_class=HTMLResponse)
async def read_root(request: Request):
    london_time = datetime.datetime.now(pytz.timezone('Europe/London'))
    current_time = london_time.strftime("%I:%M %p")  # Format time as HH:MM AM/PM
    return templates.TemplateResponse("main.html", {"request": request, "current_time": current_time})

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8000)
