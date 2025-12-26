// Objects - demonstrates object creation and methods

// Constructor function
function Person(name, age) {
    this.name = name;
    this.age = age;
}

var alice = new Person("Alice", 30);
var bob = new Person("Bob", 25);

print("Person 1: " + alice.name + ", age " + alice.age);
print("Person 2: " + bob.name + ", age " + bob.age);

// Point constructor
function Point(x, y) {
    this.x = x;
    this.y = y;
}

var p1 = new Point(3, 4);
var p2 = new Point(10, 20);

print("\nPoint 1: (" + p1.x + ", " + p1.y + ")");
print("Point 2: (" + p2.x + ", " + p2.y + ")");

// Modifying object properties
p1.x = 100;
print("Modified Point 1: (" + p1.x + ", " + p1.y + ")");

// Adding properties dynamically
p1.z = 50;
print("Added z: " + p1.z);

// instanceof check
print("\np1 instanceof Point: " + (p1 instanceof Point));
print("alice instanceof Person: " + (alice instanceof Person));
print("alice instanceof Point: " + (alice instanceof Point));
