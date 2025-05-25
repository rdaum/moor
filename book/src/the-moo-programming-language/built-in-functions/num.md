## Basic Numeric Functions

### `abs`

**Description:** Returns the absolute value of a number.
**Arguments:**

- `number`: An integer or float value

**Returns:** The absolute value of the input number, in the same type as the input

### `min`

**Description:** Returns the smallest value from the provided arguments.
**Arguments:**

- `value1, value2, ...`: Two or more values of the same type

**Returns:** The minimum value from the provided arguments
**Note:** All arguments must be of the same type.

### `max`

**Description:** Returns the largest value from the provided arguments.
**Arguments:**

- `value1, value2, ...`: Two or more values of the same type

**Returns:** The maximum value from the provided arguments
**Note:** All arguments must be of the same type.

### `random`

**Description:** Generates a random integer.
**Arguments:**

- `limit`: Optional maximum value (inclusive)

**Returns:** A random integer between 1 and the limit (or between 1 and 2,147,483,647 if no limit is provided)
**Note:** If a limit is provided, it must be a positive integer.

### `floatstr`

**Description:** Formats a floating-point number as a string with specified precision.
**Arguments:**

- `number`: The float to format
- : The number of decimal places to include `precision`
- : Optional boolean to use scientific notation (default: false) `scientific`

**Returns:** A string representation of the number

## Trigonometric Functions

### `sin`

**Description:** Calculates the sine of an angle (in radians).
**Arguments:**

- `angle`: Angle in radians

**Returns:** The sine of the angle as a float

### `cos`

**Description:** Calculates the cosine of an angle (in radians).
**Arguments:**

- `angle`: Angle in radians

**Returns:** The cosine of the angle as a float

### `tan`

**Description:** Calculates the tangent of an angle (in radians).
**Arguments:**

- `angle`: Angle in radians

**Returns:** The tangent of the angle as a float

### `asin`

**Description:** Calculates the arc sine (inverse sine) of a value.
**Arguments:**

- : A value between -1 and 1 `value`

**Returns:** The arc sine in radians as a float
**Note:** Raises E_ARGS if the value is outside the range \[-1, 1\].

### `acos`

**Description:** Calculates the arc cosine (inverse cosine) of a value.
**Arguments:**

- : A value between -1 and 1 `value`

**Returns:** The arc cosine in radians as a float
**Note:** Raises E_ARGS if the value is outside the range \[-1, 1\].

### `atan`

**Description:** Calculates the arc tangent of y/x.
**Arguments:**

- : The x coordinate `x`
- : The y coordinate `y`

**Returns:** The arc tangent of y/x in radians as a float

## Hyperbolic Functions

### `sinh`

**Description:** Calculates the hyperbolic sine of a value.
**Arguments:**

- : The input value `value`

**Returns:** The hyperbolic sine as a float

### `cosh`

**Description:** Calculates the hyperbolic cosine of a value.
**Arguments:**

- : The input value `value`

**Returns:** The hyperbolic cosine as a float

### `tanh`

**Description:** Calculates the hyperbolic tangent of a value.
**Arguments:**

- : The input value `value`

**Returns:** The hyperbolic tangent as a float

## Exponential and Logarithmic Functions

### `exp`

**Description:** Calculates e raised to the power of the input value.
**Arguments:**

- : The exponent `value`

**Returns:** e^value as a float

### `log`

**Description:** Calculates the natural logarithm (base e) of a value.
**Arguments:**

- : A positive number `value`

**Returns:** The natural logarithm as a float
**Note:** Raises E_ARGS if the value is less than or equal to 0.

### `log10`

**Description:** Calculates the base-10 logarithm of a value.
**Arguments:**

- : A positive number `value`

**Returns:** The base-10 logarithm as a float
**Note:** Raises E_ARGS if the value is less than or equal to 0.

### `sqrt`

**Description:** Calculates the square root of a value.
**Arguments:**

- : A non-negative number `value`

**Returns:** The square root as a float
**Note:** Raises E_ARGS if the value is negative.

## Rounding Functions

### `ceil`

**Description:** Rounds a value up to the nearest integer.
**Arguments:**

- : The value to round up `value`

**Returns:** The ceiling value as a float

### `floor`

**Description:** Rounds a value down to the nearest integer.
**Arguments:**

- : The value to round down `value`

**Returns:** The floor value as a float

### `trunc`

**Description:** Truncates a value toward zero.
**Arguments:**

- : The value to truncate `value`

**Returns:** The truncated value as a float
Note: All numeric functions accept both integer and float arguments and typically return results as floating-point
numbers. Functions that require specific ranges or positive values will raise appropriate errors if these conditions are
not met.
