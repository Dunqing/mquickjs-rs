// JSON - demonstrates JSON parsing and stringification

// Parse JSON
var jsonStr = '{"name": "Bob", "age": 25, "active": true}';
var obj = JSON.parse(jsonStr);

print("Parsed JSON:");
print("  Name: " + obj.name);
print("  Age: " + obj.age);
print("  Active: " + obj.active);

// Parse array
var arrStr = '[1, 2, 3, 4, 5]';
var arr = JSON.parse(arrStr);
print("\nParsed array length: " + arr.length);
print("  arr[0] = " + arr[0]);
print("  arr[4] = " + arr[4]);

// Parse nested
var nestedStr = '{"user": {"name": "Alice", "score": 95}}';
var nested = JSON.parse(nestedStr);
print("\nNested user: " + nested.user.name);
print("Nested score: " + nested.user.score);

// Stringify primitives
print("\nStringify primitives:");
print("  42: " + JSON.stringify(42));
print("  true: " + JSON.stringify(true));
print("  null: " + JSON.stringify(null));
print('  "hello": ' + JSON.stringify("hello"));

// Stringify array
print("\nStringify array:");
print("  " + JSON.stringify(arr));
