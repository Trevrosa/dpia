# everything in mA 

MAX_LOAD = 300
BAT_VOLTAGE = 11.1 # V
BAT_CAPACITY = 20 # Wh

# returns mA
def uA(a: float | list[float]) -> list[float]:
    if isinstance(a, list):
        return [x * 1e-3 for x in a]
    return [a * 1e-3]

def select(a: list[float]) -> float:
    if pessimism == 0:
        return min(a)
    elif pessimism == 1:
        return sum(a) / len(a)
    else:
        return max(a)

class Sensor:
    def __init__(self, name: str, startup: list[float], idle: list[float], measurement: list[float]):
        """"values are possible current draws in mA"""
        self.name = name
        self.startup = startup
        self.idle = idle
        self.measurement = measurement

class Load:
    def __init__(self, name: str, max: float, expected: float | None = None):
        self.name = name
        if expected is None:
            self.expected = max
        else:
            self.expected = expected
        self.max = max

loads = [
    Sensor(name="sts4x", startup=uA(50), idle=uA([0.08, 1.0, 3.4]), measurement=uA([320, 500])),
    Sensor(name="sht5x", startup=uA(50), idle=uA([0.08, 1.0, 3.4]), measurement=uA([320, 500])),
    Sensor(name="sen54", startup=[0.7, 1], idle=[0.7, 1], measurement=[70, 100]),
    Load(name="7 segment displays", max=10*7*7, expected=10*5*7),
    Load(name="pi", max=50, expected=50),
    Load(name="leds", max=30), 
]

# can be 0, 1, or 2, where 0 is optimistic and 2 is pessimistic
pessimism = 1

sensors_idle = [select(load.idle) for load in loads if isinstance(load, Sensor)]
sensors_peak = [select(load.measurement) for load in loads if isinstance(load, Sensor)]

idle = 0
peak = 0

for load in loads:
    if isinstance(load, Sensor):
        print(f"{load.name} startup: {max(load.startup):.2f} mA")
        print(f"{load.name} idle: {max(load.idle):.4f} mA")
        print(f"{load.name} measurement: {max(load.measurement):.2f} mA")
        idle += select(load.idle)
        peak += select(load.measurement)
    else:
        print(f"{load.name}: max {load.max:.2f} mA, expected {load.expected:.2f} mA)")
        idle += load.expected * pessimism * 0.2 + 0.8
        peak += load.max * pessimism * 0.2 + 0.8
    print()

def watts(mA: float) -> float:
    # Power (W) = Current (A) * Voltage (V)
    current = mA / 1000  # convert mA to A
    power = current * BAT_VOLTAGE
    return power

print(f"Total idle: {idle:.2f} mA ({watts(idle):.2f} W)")
print(f"Total peak: {peak:.2f} mA ({watts(peak):.2f} W)")

print(f"\nSensor idle: {sum(sensors_idle):.2f} mA ({watts(sum(sensors_idle)):.2f} W)")
print(f"Sensor peak: {sum(sensors_peak):.2f} mA ({watts(sum(sensors_peak)):.2f} W)")

print(f"\nWith battery capacity of {BAT_CAPACITY} Wh and voltage of {BAT_VOLTAGE} V:")
print(f"\tBattery life at idle: {BAT_CAPACITY / watts(idle):.2f} hours")
print(f"\tBattery life at peak: {BAT_CAPACITY / watts(peak):.2f} hours") 