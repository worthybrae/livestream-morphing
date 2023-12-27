import requests

def temperature_to_color(temperature):
    """ Map temperature to a color. Colder temperatures are more blue, warmer are more red. """
    # Define temperature bounds (adjust as needed)
    cold_temp = 0  # degrees Celsius
    warm_temp = 30  # degrees Celsius

    # Normalize temperature within the range
    normalized_temp = (temperature - cold_temp) / (warm_temp - cold_temp)

    # Map to color
    blue = max(128, 255 - int(127 * normalized_temp))
    red = max(128, int(127 * normalized_temp) + 128)
    green = 128

    return (blue, green, red)

def get_current_temperature():
    url = "http://api.openweathermap.org/data/2.5/weather"
    params = {
        'q': 'London,UK',
        'appid': '489d3d33334f891c0aa0fe78bbed9e80',
        'units': 'metric'  # Temperature in Celsius
    }

    response = requests.get(url, params=params)
    data = response.json()

    # Extracting temperature
    temperature = data['main']['temp']
    return temperature
