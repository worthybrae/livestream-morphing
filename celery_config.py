BROKER_URL = 'redis://localhost:6379/0'
RESULT_BACKEND = 'redis://localhost:6379/0'
CELERY_IMPORTS = ('tasks', )
CELERYBEAT_SCHEDULE = {
    'fetch-new-segments-task': {
        'task': 'tasks.fetch_new_segments',
        'schedule': 2.0,
    }
}
CELERY_ACCEPT_CONTENT = ['pickle', 'json']
CELERY_TASK_SERIALIZER = 'pickle'
CELERY_RESULT_SERIALIZER = 'pickle'
CELERY_ROUTES = {
    'tasks.fetch_new_segments': {'queue': 'fns'},
    'tasks.download_segments': {'queue': 'ds'},
    'tasks.process_segment': {'queue': 'ps'},
}