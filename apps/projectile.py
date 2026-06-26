import math


def deg2rad(d):
    return d * math.pi / 180


v = 30
angle = 45
g = 9.81
rad = deg2rad(angle)
vx = v * math.cos(rad)
vy = v * math.sin(rad)
t_flight = 2 * vy / g
rng = vx * t_flight
max_h = vy * vy / (2 * g)
print("Projectile motion (v=30 m/s, angle=45 deg)")
print(f"Time of flight: {round(t_flight, 3)} s")
print(f"Range: {round(rng, 3)} m")
print(f"Max height: {round(max_h, 3)} m")
print("Trajectory (t, height):")
for i in range(6):
    t = t_flight * i / 5
    h = vy * t - 0.5 * g * t * t
    print(f"  t={round(t, 2)} h={round(h, 3)}")
