#!/usr/bin/env python3
"""Sample a cached Abbey Road forecast into optional mineral palette parameters.

This local command never updates the production pipeline. Forecast data are not
street-level observations. MET Norway data: CC BY 4.0, adapted into color signals.
"""
import argparse
import datetime as dt
import email.utils
import gzip
import json
from pathlib import Path
import urllib.error
import urllib.request
from zoneinfo import ZoneInfo

URL = 'https://api.met.no/weatherapi/locationforecast/2.0/compact?lat=51.5321&lon=-0.1774'
USER_AGENT = 'LivingInkStudies/1.0 github.com/worthybrae/livestream-morphing'


def palette_inputs(payload, now):
    london = now.astimezone(ZoneInfo('Europe/London'))
    params = dict(edition=18, detail=1, pigment=1, hour=london.hour+london.minute/60,
                  cloud=0.3, rain=0, color_motion=0.25)
    entries = payload.get('properties', {}).get('timeseries', []) if payload else []
    if not entries:
        return params, None
    stamp = lambda entry: dt.datetime.fromisoformat(entry['time'].replace('Z', '+00:00'))
    closest = min(entries, key=lambda entry: abs((stamp(entry)-now).total_seconds()))
    if abs((stamp(closest)-now).total_seconds()) > 7200:
        return params, None
    data = closest['data']
    cloud = data.get('instant', {}).get('details', {}).get('cloud_area_fraction')
    rain = data.get('next_1_hours', {}).get('details', {}).get('precipitation_amount')
    if cloud is not None:
        params['cloud'] = max(0, min(1, cloud/100))
    if rain is not None:
        params['rain'] = max(0, min(1, rain/2))
    return params, {'forecast_time': closest['time'], 'cloud_percent': cloud,
                    'next_hour_precipitation_mm': rain}


def sample(cache_path, now=None):
    now = now or dt.datetime.now(dt.timezone.utc)
    stamp = now.timestamp()
    try:
        cache = json.loads(cache_path.read_text())
    except (FileNotFoundError, ValueError):
        cache = {}
    problem = None
    if stamp >= cache.get('next_check', 0):
        headers = {'User-Agent': USER_AGENT, 'Accept-Encoding': 'gzip'}
        if cache.get('modified'):
            headers['If-Modified-Since'] = cache['modified']
        try:
            try:
                response = urllib.request.urlopen(urllib.request.Request(URL, headers=headers), timeout=10)
            except urllib.error.HTTPError as error:
                if error.code != 304:
                    raise
                response = error
            with response:
                if response.code != 304:
                    raw = response.read()
                    if response.headers.get('Content-Encoding') == 'gzip':
                        raw = gzip.decompress(raw)
                    cache['payload'] = json.loads(raw)
                    cache['retrieved_at'] = now.isoformat()
                    cache['modified'] = response.headers.get('Last-Modified')
                expires = response.headers.get('Expires')
                expiry = email.utils.parsedate_to_datetime(expires).timestamp() if expires else stamp+3600
                cache['next_check'] = max(stamp+900, expiry)
                cache.pop('last_error', None)
        except (urllib.error.URLError, TimeoutError, OSError, ValueError) as error:
            problem = str(error)
            cache['last_error'] = problem
            cache['next_check'] = stamp+900
        cache_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = cache_path.with_suffix('.tmp')
        temporary.write_text(json.dumps(cache))
        temporary.replace(cache_path)
    params, forecast = palette_inputs(cache.get('payload'), now)
    return params, {'sampled_at': now.isoformat(), 'retrieved_at': cache.get('retrieved_at'),
                    'forecast': forecast, 'available': forecast is not None,
                    'warning': problem or cache.get('last_error'), 'source': URL,
                    'attribution': 'Weather forecast: MET Norway, CC BY 4.0; adapted into palette signals.',
                    'license': 'https://creativecommons.org/licenses/by/4.0/',
                    'next_check': cache.get('next_check')}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--cache', type=Path, required=True)
    args = parser.parse_args()
    params, status = sample(args.cache)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(params, indent=2))
    args.output.with_suffix('.status.json').write_text(json.dumps(status, indent=2))
    print(json.dumps({'params': params, 'status': status}, indent=2))
