from fastapi import FastAPI
from fastapi.responses import StreamingResponse, HTMLResponse
import boto3
import datetime
import os
from dotenv import load_dotenv
import pytz

load_dotenv(override=True)

app = FastAPI()
s3 = boto3.client(
    's3',
    aws_access_key_id=os.getenv('AWS_PUB_KEY'),
    aws_secret_access_key=os.getenv('AWS_SECRET_KEY'),
)

@app.get("/stream")
async def stream():
    try:
        london_time = datetime.datetime.now(pytz.timezone('Europe/London')) - datetime.timedelta(seconds=180)
        playlist_key = f'playlists/{london_time.hour}/{london_time.minute}.m3u8'
        print(playlist_key)

        # Fetch the playlist file from S3
        playlist = s3.get_object(Bucket='abbey-road', Key=playlist_key)['Body'].read()
        return StreamingResponse(playlist, media_type='application/x-mpegURL')
    except Exception as e:
        print(f"Error fetching the playlist: {e}")
        return StreamingResponse(b"Error fetching the playlist", status_code=500)

@app.get("/", response_class=HTMLResponse)
async def main():
    content = """
    <!DOCTYPE html>
    <html>
    <head>
    <style>
        body {
        margin: 0;
        height: 100vh;
        overflow: hidden;
        }
        video {
        object-fit: cover;
        width: 100vw;
        height: 100vh;
        position: fixed;
        top: 0;
        left: 0;
        }
    </style>
    </head>
    <body>
    <video autoplay loop muted playsinline>
        <source src="/stream" type="application/x-mpegURL">
        Your browser does not support the video tag.
    </video>
    </body>
    </html>
    """
    return HTMLResponse(content=content)