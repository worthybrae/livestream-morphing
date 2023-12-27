# Livestream Morphing

## Setup Instructions

```
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

## first_iteration.py

This was the first attempt at conducting livestream morphing. It processes three segments at a time and doesnt use background segment fetching. Due to this, there is significant lag introduced while waiting to process the three segments.

## second_iteration.py

This was the second attempt at conducting livestream morphing. It runs two processes in parallel:

1. Fetches up to 7 segments asynchronously and uses background fetching to constantly check if new segments are available.
2. Processes each of the frames from each segment asynchronously and saves them locally.

When displaying the frames, the video player can simply run through all the processed frames locally saved in the frames sub-folder.

## api.py

This is the main fastapi file that is responsible for delivering the processed frames via an api endpoint. While not operational now, this will be once the morphing process is finalized.

## temperature_helpers.py

This is a small python file that explores using the current temperature in a specified location to modify frame tint.
