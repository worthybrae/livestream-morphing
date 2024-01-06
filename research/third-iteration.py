import time
import requests
import m3u8
import redis

# Connect to Redis server
redis_client = redis.Redis(host='localhost', port=6379, db=0)

# Global Vars
buffer_segments = 5


def get_segment_number(ts_url):
    try:
        return int(ts_url.split('_')[-1].split('.')[0])
    except ValueError:
        return float('inf')

def get_active_segments():
    epoch_time = int(time.time())
    url = f"https://videos-3.earthcam.com/fecnetwork/AbbeyRoadHD1.flv/chunklist_w{epoch_time}.m3u8"
    headers = {
        'Accept': '*/*',
        'Accept-Language': 'en-US,en;q=0.9',
        'Connection': 'keep-alive',
        'Origin': 'https://www.abbeyroad.com',
        'Referer': 'https://www.abbeyroad.com/',
        'Sec-Fetch-Dest': 'empty',
        'Sec-Fetch-Mode': 'cors',
        'Sec-Fetch-Site': 'cross-site',
        'User-Agent': 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36',
        'sec-ch-ua': '"Google Chrome";v="119", "Chromium";v="119", "Not?A_Brand";v="24"',
        'sec-ch-ua-mobile': '?0',
        'sec-ch-ua-platform': '"macOS"'
    }
    m3u8_response = requests.get(url, headers=headers)
    playlist = m3u8.loads(m3u8_response.text)
    if playlist.segments:
        max_segment_number = max(get_segment_number(segment.uri) for segment in playlist.segments)
        min_segment_number = max(max_segment_number - buffer_segments, 0)

        for s in range(min_segment_number, max_segment_number):
            print(redis_client.zscore('active_segments', s))
            redis_client.zadd('active_segments', {"New": s})

        redis_client.zremrangebyrank('active_segments', 0, -6)
        return True

    else:
        return False
    


while True:
    get_active_segments()
    time.sleep(2)