import os
from dotenv import load_dotenv
import boto3

load_dotenv(override=True)


s3 = boto3.client(
    's3',
    aws_access_key_id=os.getenv('AWS_PUB_KEY'),
    aws_secret_access_key=os.getenv('AWS_SECRET_KEY'),
)
signed_url = s3.generate_presigned_url(
    'get_object',
    Params={'Bucket': 'abbey-road', 'Key': 'playlists/0/9.m3u8'},
    ExpiresIn=600
)
print(signed_url)