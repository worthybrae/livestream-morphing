import uvicorn
import time
import os
import asyncio
from fastapi import FastAPI, Request
from fastapi.responses import StreamingResponse, HTMLResponse
from fastapi.templating import Jinja2Templates

class FrameFetcher:
    def __init__(self):
        self.frame = 0
        self.segment = 0
        self.current_frame_content = None

    async def update_frame(self):
        if self._get_first_segment():
            while True:
                print(f"Segment: {self.segment}\tFrame: {self.frame}")
                try:
                    with open(f'frames/{self.segment}/{self.frame}.jpg', "rb") as f:
                        self.current_frame_content = f.read()
                    await asyncio.sleep(1/29)  # Wait for the next frame
                except FileNotFoundError:
                    await asyncio.sleep(1/29)  # Wait if the frame doesn't exist
                self._get_next_frame()
            
    def _get_first_segment(self):
        time.sleep(20)
        segments = [d for d in os.listdir('segments')]
        segments.sort(reverse=True)
        self.segment = int(segments[2])
        self.frame = 0
        return True

    def _get_next_frame(self):
        if self.frame == 179:
            self.segment += 1
            self.frame = 0
        else:
            self.frame += 1

app = FastAPI()
templates = Jinja2Templates(directory="templates")
frame_fetcher = FrameFetcher()

@app.on_event("startup")
async def startup_event():
    asyncio.create_task(frame_fetcher.update_frame())

@app.get("/stream")
def stream_frames():
    def generate():
        while True:
            frame_data = frame_fetcher.current_frame_content
            if frame_data:
                yield (b'--frame\r\n'
                       b'Content-Type: image/jpeg\r\n\r\n' + frame_data + b'\r\n')
            else:
                time.sleep(1/30)  # If no frame data, wait for the next frame

    return StreamingResponse(generate(), media_type="multipart/x-mixed-replace; boundary=frame")

@app.get("/", response_class=HTMLResponse)
async def read_root(request: Request):
    return templates.TemplateResponse("main.html", {"request": request})

if __name__ == "__main__":
    uvicorn.run(app, host="0.0.0.0", port=8000)

