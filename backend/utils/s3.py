"""
S3/R2 upload utilities.
Handles uploading processed segments and playlists to cloud storage.
"""
import os
import boto3
from dotenv import load_dotenv

load_dotenv(override=True)

# Initialize S3 client
def get_s3_client():
    """
    Creates and returns an S3 client.
    Supports both AWS S3 and Cloudflare R2.
    """
    # Check if using Cloudflare R2
    r2_account_id = os.getenv('R2_ACCOUNT_ID')

    if r2_account_id:
        # Cloudflare R2 configuration
        return boto3.client(
            's3',
            endpoint_url=f'https://{r2_account_id}.r2.cloudflarestorage.com',
            aws_access_key_id=os.getenv('R2_ACCESS_KEY'),
            aws_secret_access_key=os.getenv('R2_SECRET_KEY'),
            region_name='auto'
        )
    else:
        # AWS S3 configuration
        return boto3.client(
            's3',
            aws_access_key_id=os.getenv('AWS_PUB_KEY'),
            aws_secret_access_key=os.getenv('AWS_SECRET_KEY'),
            region_name=os.getenv('AWS_REGION', 'us-east-1')
        )


def upload_to_s3(file_data, object_name, bucket_name=None):
    """
    Uploads a file to S3 or R2.

    Args:
        file_data: File-like object (BytesIO) or bytes to upload
        object_name: Key/path in the bucket (e.g., "segments/12345.ts")
        bucket_name: Optional bucket name (defaults to env var)

    Returns:
        Response dict on success, None on failure
    """
    if bucket_name is None:
        bucket_name = os.getenv('S3_BUCKET', 'abbey-road')

    s3 = get_s3_client()

    try:
        # Ensure file_data is at the beginning
        if hasattr(file_data, 'seek'):
            file_data.seek(0)

        # Upload the file
        response = s3.upload_fileobj(file_data, bucket_name, object_name)
        # print(f"✅ Uploaded {object_name} to {bucket_name}")
        return response

    except Exception as e:
        print(f"❌ Failed to upload {object_name} to {bucket_name}: {e}")
        return None


def download_from_s3(object_name, bucket_name=None):
    """
    Downloads a file from S3 or R2.

    Args:
        object_name: Key/path in the bucket
        bucket_name: Optional bucket name (defaults to env var)

    Returns:
        File content as bytes, or None on failure
    """
    if bucket_name is None:
        bucket_name = os.getenv('S3_BUCKET', 'abbey-road')

    s3 = get_s3_client()

    try:
        response = s3.get_object(Bucket=bucket_name, Key=object_name)
        return response['Body'].read()
    except Exception as e:
        print(f"❌ Failed to download {object_name} from {bucket_name}: {e}")
        return None
