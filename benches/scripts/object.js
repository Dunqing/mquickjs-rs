// Object benchmark - tests object property access
function Point(x, y) {
    this.x = x;
    this.y = y;
}

var points = [];
for (var i = 0; i < 10000; i = i + 1) {
    points.push(new Point(i, i * 2));
}

var sumX = 0;
var sumY = 0;
for (var i = 0; i < points.length; i = i + 1) {
    sumX = sumX + points[i].x;
    sumY = sumY + points[i].y;
}
print("sumX = " + sumX + ", sumY = " + sumY);
