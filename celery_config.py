# FOR PRODUCTION
# broker_url = 'redis://redis:6379/0'
# result_backend = 'redis://redis:6379/0'

# FOR LOCAL TESTING
broker_url = 'redis://127.0.0.1:6379/0'
result_backend = 'redis://127.0.0.1:6379/0'

imports = ('tasks', )
accept_content = ['pickle', 'json']
task_serializer = 'pickle'
result_serializer = 'pickle'
task_routes = {
    'tasks.fetch_new_segment': {'queue': 'fns'},
    'tasks.download_segments': {'queue': 'ds'},
    'tasks.process_segment': {'queue': 'ps'},
    'tasks.generate_m3u8_file': {'queue': 'gm'},
}