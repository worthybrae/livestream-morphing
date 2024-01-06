# Livestream Morphing

## Summary

```
├── research/
│   ├── app.py - an intitial try at a celery app file
│   ├── first_iteration.py - uses async requests to process frames
│   ├── process_frame.py - code used to process many frames concurrently
│   ├── process_sketch.drawio - draw.io file used to visualize process
│   ├── redis-test.py - code used to test out a redis implimentation
│   ├── scripts.py - consolidated and final scripts
│   ├── second_iteration.py - uses async requests dynamic storage to process and view frames
│   ├── temperature_helpers.py - code used to identify the temperature in a target location
│   └── third-iteration.py - final version of code that was used for the scripts.py file
├── templates/
│   └── main.html - the first template file used to style the livestream webpage
├── .gitignore
├── api.py - fastapi app used for visualizing the processed frames
├── app.py - celery app implimentation with final version of tasks
├── ReadMe.md
└── requirements.txt
```

## Setup Instructions

### Setting up Virtual Enviornment

```
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

### Starting up the Celery Backend

```
celery -A app beat --loglevel=info
celery -A app worker --loglevel=info --concurrency=5 -Q download_queue
celery -A app worker --loglevel=info --concurrency=5 -Q process_queue
celery -A app flower
```

### Starting up the FastAPI

```
uvicorn api:app --reload
```
