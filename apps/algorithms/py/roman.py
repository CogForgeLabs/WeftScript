def to_roman(n):
    vals = [1000, 900, 500, 400, 100, 90, 50, 40, 10, 9, 5, 4, 1]
    syms = ["M", "CM", "D", "CD", "C", "XC", "L", "XL", "X", "IX", "V", "IV", "I"]
    out = ""
    i = 0
    while i < len(vals):
        while n >= vals[i]:
            out = out + syms[i]
            n = n - vals[i]
        i += 1
    return out

print(f"2024 = {to_roman(2024)}")
print(f"49 = {to_roman(49)}")
print(f"1994 = {to_roman(1994)}")
