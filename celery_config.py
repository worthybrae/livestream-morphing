broker_url = 'redis://redis:6379/0'
result_backend = 'redis://redis:6379/0'
imports = ('tasks', )
beat_schedule = {
    'fetch-new-segments-task': {
        'task': 'tasks.fetch_new_segments',
        'schedule': 2.0,
    }
}
accept_content = ['pickle', 'json']
task_serializer = 'pickle'
result_serializer = 'pickle'
task_routes = {
    'tasks.fetch_new_segments': {'queue': 'fns'},
    'tasks.download_segments': {'queue': 'ds'},
    'tasks.process_segment': {'queue': 'ps'},
}