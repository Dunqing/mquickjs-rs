// TypedArrays - demonstrates typed array operations

// Create typed arrays
var int8 = new Int8Array(4);
int8[0] = 10;
int8[1] = 20;
int8[2] = 30;
int8[3] = 127;

print("Int8Array: " + int8[0] + ", " + int8[1] + ", " + int8[2] + ", " + int8[3]);
print("  Length: " + int8.length);
print("  ByteLength: " + int8.byteLength);
print("  BYTES_PER_ELEMENT: " + int8.BYTES_PER_ELEMENT);

// Overflow behavior
var overflow = new Int8Array(2);
overflow[0] = 128;  // Wraps to -128
overflow[1] = 255;  // Wraps to -1
print("\nInt8 overflow: 128 -> " + overflow[0] + ", 255 -> " + overflow[1]);

// Uint8ClampedArray (used for canvas pixel data)
var clamped = new Uint8ClampedArray(3);
clamped[0] = -10;   // Clamped to 0
clamped[1] = 300;   // Clamped to 255
clamped[2] = 100;   // Stays 100
print("\nUint8Clamped: " + clamped[0] + ", " + clamped[1] + ", " + clamped[2]);

// Larger integer types
var int32 = new Int32Array(2);
int32[0] = 1000000;
int32[1] = -1000000;
print("\nInt32Array: " + int32[0] + ", " + int32[1]);

// Create from array
var fromArray = new Uint8Array([1, 2, 3, 4, 5]);
print("\nFrom array: " + fromArray[0] + ", " + fromArray[1] + ", " + fromArray[2]);

// Subarray (view into same buffer)
var sub = fromArray.subarray(1, 4);
print("Subarray(1,4): " + sub[0] + ", " + sub[1] + ", " + sub[2]);
