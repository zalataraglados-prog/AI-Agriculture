import time
from machine import Pin
import dht

DHT_PIN = 22
SENSOR_KIND = "DHT22"
READ_INTERVAL_SEC = 5
RETRY_COUNT = 3
RETRY_DELAY_SEC = 1

if SENSOR_KIND == "DHT22":
    sensor = dht.DHT22(Pin(DHT_PIN))
elif SENSOR_KIND == "DHT11":
    sensor = dht.DHT11(Pin(DHT_PIN))
else:
    raise ValueError("Unsupported SENSOR_KIND: {}".format(SENSOR_KIND))

print("{} dedicated reader start".format(SENSOR_KIND))
print(
    "pin={} interval={}s retries={}".format(
        DHT_PIN, READ_INTERVAL_SEC, RETRY_COUNT
    )
)

while True:
    reading_ok = False
    last_err = None

    for attempt in range(1, RETRY_COUNT + 1):
        try:
            sensor.measure()
            temp_c = sensor.temperature()
            hum = sensor.humidity()
            print("{} temp_c={} hum={}".format(SENSOR_KIND, temp_c, hum))
            reading_ok = True
            break
        except Exception as err:
            last_err = err
            print("{} read_fail attempt={} err={}".format(SENSOR_KIND, attempt, err))
            time.sleep(RETRY_DELAY_SEC)

    if not reading_ok:
        print("{} read_fail final err={}".format(SENSOR_KIND, last_err))

    time.sleep(READ_INTERVAL_SEC)
