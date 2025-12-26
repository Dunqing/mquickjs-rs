// Closures - demonstrates variable capture

function makeCounter(start) {
    var count = start;
    function counter() {
        count = count + 1;
        return count;
    }
    return counter;
}

var counter1 = makeCounter(0);
var counter2 = makeCounter(100);

print("Counter 1: " + counter1() + ", " + counter1() + ", " + counter1());
print("Counter 2: " + counter2() + ", " + counter2() + ", " + counter2());

// Closure with captured parameter
function makeAdder(x) {
    function adder(y) {
        return x + y;
    }
    return adder;
}

var add5 = makeAdder(5);
var add10 = makeAdder(10);

print("add5(3) = " + add5(3));
print("add10(3) = " + add10(3));

// Multiple captured variables
function outer() {
    var a = 10;
    var b = 20;
    function inner() {
        return a + b;
    }
    return inner();
}

print("outer() with a=10, b=20: " + outer());
