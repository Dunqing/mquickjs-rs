// Arrays - demonstrates array methods

var numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

print("Original array length: " + numbers.length);

// Access elements
print("\nElement access:");
print("numbers[0] = " + numbers[0]);
print("numbers[4] = " + numbers[4]);
print("numbers[9] = " + numbers[9]);

// Basic operations
print("\nPush/Pop:");
var arr = [1, 2, 3];
arr.push(4);
print("After push(4), length: " + arr.length + ", last: " + arr[3]);
var popped = arr.pop();
print("Popped: " + popped + ", new length: " + arr.length);

// Slice
print("\nSlice:");
var sliced = numbers.slice(2, 5);
print("slice(2, 5) length: " + sliced.length);
print("slice(2, 5)[0] = " + sliced[0] + ", [1] = " + sliced[1] + ", [2] = " + sliced[2]);

// Concat
print("\nConcat:");
var a = [1, 2];
var b = [3, 4];
var c = a.concat(b);
print("Concatenated length: " + c.length);

// Reverse
print("\nReverse:");
var toReverse = [1, 2, 3];
toReverse.reverse();
print("Reversed: " + toReverse[0] + ", " + toReverse[1] + ", " + toReverse[2]);

// indexOf
print("\nindexOf:");
print("indexOf(5): " + numbers.indexOf(5));
print("indexOf(99): " + numbers.indexOf(99));

// includes
print("\nincludes:");
print("includes(5): " + numbers.includes(5));
print("includes(99): " + numbers.includes(99));

// For loop iteration
print("\nFor loop sum:");
var sum = 0;
for (var i = 0; i < numbers.length; i = i + 1) {
    sum = sum + numbers[i];
}
print("Sum of 1-10: " + sum);
