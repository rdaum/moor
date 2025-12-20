## Basic Numeric Functions

### `abs`

```
int abs(int x)
```

Returns the absolute value of x.

If x is negative, then the result is `-x`; otherwise, the result is x. The number x can be either integer or
floating-point; the result is of the same kind.

### `min`

```
int min(int x, ...)
```

Return the smallest of it's arguments.

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is
raised.

### `max`

```
int max(int x, ...)
```

Return the largest of it's arguments.

All of the arguments must be numbers of the same kind (i.e., either integer or floating-point); otherwise `E_TYPE` is
raised.

### `random`

```
int random([int mod, [int range]])
```

random -- Return a random integer

mod must be a positive integer; otherwise, `E_INVARG` is raised. If mod is not provided, it defaults to the largest MOO
integer, which will depend on if you are running 32 or 64-bit.

if range is provided then an integer in the range of mod to range (inclusive) is returned.

```
random(10)                  => integer between 1-10
random()                    => integer between 1 and maximum integer supported
random(1, 5000)             => integer between 1 and 5000
```

### `floatstr`

```
str floatstr(float x, int precision [, scientific])
```

Converts x into a string with more control than provided by either `tostr()` or `toliteral()`.

Precision is the number of digits to appear to the right of the decimal point, capped at 4 more than the maximum
available precision, a total of 19 on most machines; this makes it possible to avoid rounding errors if the resulting
string is subsequently read back as a floating-point value. If scientific is false or not provided, the result is a
string in the form `"MMMMMMM.DDDDDD"`, preceded by a minus sign if and only if x is negative. If scientific is provided
and true, the result is a string in the form `"M.DDDDDDe+EEE"`, again preceded by a minus sign if and only if x is
negative.

## Trigonometric Functions

### `sin`

```
float sin(float x)
```

Returns the sine of x.

### `cos`

```
float cos(float x)
```

Returns the cosine of x.

### `tan`

```
float tan(float x)
```

Returns the tangent of x.

### `asin`

```
float asin(float x)
```

Returns the arc-sine (inverse sine) of x, in the range `[-pi/2..pi/2]`

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

### `acos`

```
float acos(float x)
```

Returns the arc-cosine (inverse cosine) of x, in the range `[0..pi]`

Raises `E_INVARG` if x is outside the range `[-1.0..1.0]`.

### `atan`

```
float atan(float y [, float x])
```

Returns the arc-tangent (inverse tangent) of y in the range `[-pi/2..pi/2]`.

if x is not provided, or of `y/x` in the range `[-pi..pi]` if x is provided.

## Hyperbolic Functions

### `sinh`

```
float sinh(float x)
```

Returns the hyperbolic sine of x.

### `cosh`

```
float cosh(float x)
```

Returns the hyperbolic cosine of x.

### `tanh`

```
float tanh(float x)
```

Returns the hyperbolic tangent of x.

### `asinh`

```
float asinh(num x)
```

Returns the inverse hyperbolic sine of x.

### `acosh`

```
float acosh(num x)
```

Returns the inverse hyperbolic cosine of x.

Raises `E_INVARG` if x is less than 1.

### `atanh`

```
float atanh(num x)
```

Returns the inverse hyperbolic tangent of x.

Note: The result is undefined (NaN or infinity) if |x| >= 1.

## Exponential and Logarithmic Functions

### `exp`

```
float exp(float x)
```

Returns e raised to the power of x.

### `log`

```
float log(float x)
```

Returns the natural logarithm of x.

Raises `E_INVARG` if x is not positive.

### `log10`

```
float log10(float x)
```

Returns the base 10 logarithm of x.

Raises `E_INVARG` if x is not positive.

### `sqrt`

```
float sqrt(float x)
```

Returns the square root of x.

Raises `E_INVARG` if x is negative.

### `cbrt`

```
float cbrt(num x)
```

Returns the cube root of x. Unlike `sqrt()`, this works for negative numbers.

```
cbrt(8)    => 2.0
cbrt(-27)  => -3.0
```

### `exp2`

```
float exp2(num x)
```

Returns 2 raised to the power of x.

```
exp2(3)   => 8.0
exp2(0.5) => 1.4142135623730951
```

### `expm1`

```
float expm1(num x)
```

Returns e^x - 1 in a way that is accurate even when x is close to zero.

This is more accurate than computing `exp(x) - 1` directly for small values of x.

### `log2`

```
float log2(num x)
```

Returns the base-2 logarithm of x.

Raises `E_INVARG` if x is not positive.

```
log2(8)   => 3.0
log2(256) => 8.0
```

### `ln1p`

```
float ln1p(num x)
```

Returns ln(1+x) in a way that is accurate even when x is close to zero.

This is more accurate than computing `log(1 + x)` directly for small values of x.

Raises `E_INVARG` if x is less than or equal to -1.

### `hypot`

```
float hypot(num x, num y)
```

Returns sqrt(x² + y²), computed in a way that avoids overflow and underflow.

Useful for calculating the length of the hypotenuse of a right triangle or the distance between two points.

```
hypot(3, 4) => 5.0
```

## Angle Conversion Functions

### `to_degrees`

```
float to_degrees(num x)
```

Converts x from radians to degrees.

```
to_degrees(3.14159265358979) => 180.0
```

### `to_radians`

```
float to_radians(num x)
```

Converts x from degrees to radians.

```
to_radians(180) => 3.14159265358979
```

## Rounding Functions

### `ceil`

```
float ceil(float x)
```

Returns the smallest integer not less than x, as a floating-point number.

### `floor`

```
float floor(float x)
```

Returns the largest integer not greater than x, as a floating-point number.

### `trunc`

```
float trunc(float x)
```

Returns the integer obtained by truncating x at the decimal point, as a floating-point number.

For negative x, this is equivalent to `ceil()`; otherwise it is equivalent to `floor()`.

### `round`

```
float round(num x)
```

Returns x rounded to the nearest integer, as a floating-point number. Halfway cases round away from zero.

```
round(2.3)   => 2.0
round(2.5)   => 3.0
round(-2.5)  => -3.0
```

### `fract`

```
float fract(num x)
```

Returns the fractional part of x (equivalent to `x - trunc(x)`).

```
fract(3.14)   => 0.14
fract(-3.14)  => -0.14
```

### `signum`

```
float signum(num x)
```

Returns the sign of x: 1.0 if x is positive, -1.0 if negative, or 0.0 if zero.

```
signum(42)   => 1.0
signum(-17)  => -1.0
signum(0)    => 0.0
```

### `recip`

```
float recip(num x)
```

Returns the reciprocal (1/x) of x.

```
recip(2)   => 0.5
recip(0.5) => 2.0
```

### `copysign`

```
float copysign(num magnitude, num sign)
```

Returns a value with the magnitude of the first argument and the sign of the second.

```
copysign(5.0, -1.0)  => -5.0
copysign(-5.0, 1.0)  => 5.0
```
