"""Download and verify the optional local person detector used by the study."""
import hashlib
from pathlib import Path
import urllib.request

REVISION = '47534e27c9851bb1128ccc0102f1145e27f23f98'
SHA256 = '4b82da9944b88577175ee23a459dce2e26e6e4be573def65b1055dc2d9720186'
URL = f'https://media.githubusercontent.com/media/opencv/opencv_zoo/{REVISION}/models/object_detection_nanodet/object_detection_nanodet_2022nov.onnx'


def main():
    root = Path(__file__).parent/'models';root.mkdir(exist_ok=True)
    destination = root/'nanodet.onnx'
    if destination.exists() and hashlib.sha256(destination.read_bytes()).hexdigest()==SHA256:
        print(destination);return
    data = urllib.request.urlopen(URL,timeout=60).read()
    if hashlib.sha256(data).hexdigest()!=SHA256:
        raise RuntimeError('Person model checksum does not match the pinned artifact')
    destination.write_bytes(data)
    print(destination)


if __name__=='__main__':
    main()
