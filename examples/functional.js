// Functional Programming - demonstrates higher-order functions

// Compose functions
function compose(f, g) {
    function composed(x) {
        return f(g(x));
    }
    return composed;
}

function double(x) { return x * 2; }
function addOne(x) { return x + 1; }

var doubleThenAddOne = compose(addOne, double);
var addOneThenDouble = compose(double, addOne);

print("compose(addOne, double)(5) = " + doubleThenAddOne(5));  // (5*2)+1 = 11
print("compose(double, addOne)(5) = " + addOneThenDouble(5));  // (5+1)*2 = 12

// Simple closure - makeAdder
function makeAdder(x) {
    function adder(y) {
        return x + y;
    }
    return adder;
}

var add5 = makeAdder(5);
var add10 = makeAdder(10);

print("\nmakeAdder(5)(3) = " + add5(3));
print("makeAdder(10)(3) = " + add10(3));

// Manual array operations
function sumArray(arr) {
    var result = 0;
    for (var i = 0; i < arr.length; i = i + 1) {
        result = result + arr[i];
    }
    return result;
}

function mapDouble(arr) {
    var result = [];
    for (var i = 0; i < arr.length; i = i + 1) {
        result.push(arr[i] * 2);
    }
    return result;
}

var nums = [1, 2, 3, 4, 5];
print("\nSum of [1,2,3,4,5] = " + sumArray(nums));

var doubled = mapDouble(nums);
print("Doubled: " + doubled[0] + ", " + doubled[1] + ", " + doubled[2] + ", " + doubled[3] + ", " + doubled[4]);

// Recursive fibonacci with call count
var callCount = 0;
function slowFib(n) {
    callCount = callCount + 1;
    if (n <= 1) return n;
    return slowFib(n - 1) + slowFib(n - 2);
}

callCount = 0;
print("\nRecursive fibonacci:");
print("fib(15) = " + slowFib(15) + " (calls: " + callCount + ")");
