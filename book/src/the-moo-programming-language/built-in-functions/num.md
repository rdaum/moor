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
