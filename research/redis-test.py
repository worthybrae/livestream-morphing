import redis

# Connect to Redis server
redis_client = redis.Redis(host='localhost', port=6379, db=0)

# Set a key-value pair
redis_client.set('test_key', 'hello world')

# Get the value associated with a key
value = redis_client.get('test_key')
print(value.decode('utf-8'))
