from celery import Celery
from celery.schedules import timedelta

app = Celery('livestream_morphing',
             broker='redis://localhost:6379/0',
             backend='redis://localhost:6379/0',
             include=['scripts'])

app.conf.update(
    result_expires=3600,
)

app.conf.beat_schedule = {
    'fetch-new-segments-every-2-seconds': {
        'task': 'scripts.fetch_new_segments_task',
        'schedule': timedelta(seconds=2),
    },
}

app.conf.task_routes = {
    'scripts.download_segment': {'queue': 'download_queue'},
    'scripts.process_segment': {'queue': 'process_queue'},
}