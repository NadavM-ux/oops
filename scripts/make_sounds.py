"""Generates the two tiny UI sounds embedded in the app (no samples, pure synthesis)."""
import math, struct, wave, sys, os

RATE = 44100

def render(path, segments, volume):
    frames = []
    for (f0, f1, dur, delay) in segments:
        start = int(delay * RATE)
        n = int(dur * RATE)
        while len(frames) < start + n:
            frames.append(0.0)
        phase = 0.0
        for i in range(n):
            t = i / n
            freq = f0 * (f1 / f0) ** t                       # exponential glide
            phase += 2 * math.pi * freq / RATE
            attack = min(1.0, i / (0.004 * RATE))            # 4 ms attack, no click
            env = attack * math.exp(-5.5 * t)                # quick decay
            s = math.sin(phase) + 0.18 * math.sin(2 * phase)  # a little warmth
            frames[start + i] += s * env
    peak = max(abs(x) for x in frames) or 1
    data = b"".join(struct.pack("<h", int(x / peak * volume * 32767)) for x in frames)
    with wave.open(path, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE); w.writeframes(data)

out = sys.argv[1]
# "fixed": two rising bubbles
render(os.path.join(out, "fixed.wav"), [(620, 980, 0.075, 0.0), (930, 1480, 0.11, 0.06)], 0.32)
# "nothing to fix": one soft low bonk
render(os.path.join(out, "nothing.wav"), [(330, 210, 0.16, 0.0)], 0.30)
