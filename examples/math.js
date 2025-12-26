// Math - demonstrates Math object

print("Math constants:");
print("  PI: " + Math.PI);
print("  E: " + Math.E);
print("  SQRT2: " + Math.SQRT2);

print("\nBasic operations:");
print("  abs(-5): " + Math.abs(-5));
print("  floor(3.7): " + Math.floor(3.7));
print("  ceil(3.2): " + Math.ceil(3.2));
print("  round(3.5): " + Math.round(3.5));

print("\nPower and roots:");
print("  pow(2, 10): " + Math.pow(2, 10));
print("  sqrt(16): " + Math.sqrt(16));

print("\nMin/Max:");
print("  max(1, 5, 3): " + Math.max(1, 5, 3));
print("  min(1, 5, 3): " + Math.min(1, 5, 3));

print("\nTrigonometry:");
print("  sin(0): " + Math.sin(0));
print("  cos(0): " + Math.cos(0));
print("  sin(PI/2): " + Math.sin(Math.PI / 2));

print("\nRandom numbers:");
for (var i = 0; i < 5; i = i + 1) {
    print("  random(): " + Math.random());
}
