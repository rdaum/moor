object MATRIX_UTILS
  name: "Vector and Matrix Utils"
  parent: GENERIC_UTILS
  owner: HACKER
  readable: true

  property note (owner: HACKER, flags: "rc") = "Please contact Uther@LambdaMOO if you make changes to this object, so he can make the changes on Lambda and elsewhere.";

  override aliases = {"Vector and Matrix Utils", "vector", "matrix"};
  override description = "This is a utilities package for dealing with lists as representations of vectors and matrices. Type `help $matrix_utils' for more details.";
  override help_msg = {
    "Utility verbs for manipulating lists as vectors (one dimensional lists) or as matrices (two dimensional lists).",
    "",
    "Some definitions:",
    "A VECTOR is a list of INTs or a list of FLOATs. Each element in the list represents the vector's cartesian coordinate as measured from its tail to its tip. (For instance, {3, 4} represents a vector in the x-y plane with an x component of 3 and a y component of 4. {-2, 5, 10} represents a vector in 3-space with a x component of -2, a y component of 5 and a z component of 10.)",
    "A MATRIX is a list of VECTORs, all of which have the same number (and type) of components.",
    "",
    "Vector verbs:",
    ":vector_add        (V1 [,V2 ...]) => VN such that VN[n] = V1[n] + V2[n]...",
    ":vector_sub        (V1 [,V2 ...]) => VN such that VN[n] = V1[n] - V2[n]...",
    ":scalar_vector_mul (V, S)         => VN such that VN[n] = V[n] * S...",
    ":scalar_vector_div (V, S)         => VN such that VN[n] = V[n] / S...",
    ":dot_prod          (V1, V2)       => NUM sum of the products of the ",
    ":inner_prod                          corresponding elements of the two",
    "                                     vectors.",
    ":cross_prod        (V1, V2)       => VN, the vector perpendicular to both V1",
    ":outer_prod                          and V2 with length equal to the area of",
    "                                     the parallelogram spanned by V1 and V2.",
    ":subtended_angle   (V1, V2)       => FLOAT smallest radian angle defined by",
    "                                     V1 and V2.",
    ":length            (V)            => FLOAT length of the vector. ",
    ":norm",
    "",
    "Matrix and Vector verbs:",
    ":dimensions (M) => LIST of dimensional sizes",
    ":order      (M) => NUM of dimensions",
    "",
    "Matrix verbs:",
    ":matrix_add (M1 [,M2 ...]) => MN such that MN[m][n] = M1[m][n] + M2[m][n]...",
    ":matrix_sub (M1 [,M2 ...]) => MN such that MN[m][n] = M1[m][n] - M2[m][n]...",
    ":matrix_mul (M1, M2)       => MN such than MN[m][n] = the dot product of the  ",
    "                              mth row of M1 and the nth column of M2.",
    ":scalar_matrix_mul (M, S)  => MN such that MN[m][n] = M[m][n] * S...",
    ":scalar_matrix_div (M, S)  => MN such that MN[m][n] = M[m][n] / S...",
    ":transpose  (M1)           => M2 such that the rows in M1 are the columns in",
    "                              M2 and vice versa.",
    ":identity   (INT <size>)   => Identity matrix (I) of dimensions <size> by ",
    "                              <size>.",
    ":null       (INT <size>)   => Null matrix (O) of dimensions <size> by <size>.",
    ":is_square  (M)            => 1 iff dimensions of M are equal.",
    ":column     (M, INT <n>)   => LIST the nth column of M.",
    "",
    "Square Matrix verbs:",
    ":determinant (M) => NUM the determinant of the square matrix.",
    ":inverse     (M) => the matrix that M multiplied by :inverse(M) yields I.",
    ":is_identity (M) => 1 iff M is I.",
    ":is_null     (M) => 1 iff M is O.",
    "",
    "Relation verbs:",
    ":is_reflexive   (M) => 1 if M is a reflexive relation, -1 if areflexive,",
    "                       0 otherwise.",
    ":is_areflexive  (M) => 1 if M is an areflexive relation, -1 if reflexive,",
    "                       0 otherwise.",
    ":is_symmetric   (M) => 1 if M is a symmetric relation, -1 if asymmetric,",
    "                       0 otherwise.",
    ":is_asymmetric  (M) => 1 if M is an asymmetric relation, -1 if symmetric,",
    "                       0 otherwise.",
    ":is_transitive  (M) => 1 if M is a transitive relation, -1 if atransitive,",
    "                       0 otherwise.",
    ":is_atransitive (M) => 1 if M is an atransitive relation, -1 if transitive,",
    "                       0 otherwise.",
    ":is_partial_ordering (M) => 1 if M is a reflexive, asymmetric, transitive",
    "                            relation."
  };
  override object_size = {29765, 1084848672};

  verb "vector_add vector_sub vector_mul vector_div" (this none this) owner: HACKER flags: "rxd"
    ":vector_add(V1 [,V2 ...]) => VN such that VN[n] = V1[n] + V2[n]...";
    ":vector_sub(V1 [,V2 ...]) => VN such that VN[n] = V1[n] - V2[n]...";
    ":vector_mul(V1 [,V2 ...]) => VN such that VN[n] = V1[n] * V2[n]...";
    ":vector_div(V1 [,V2 ...]) => VN such that VN[n] = V1[n] / V2[n]...";
    "Vectors do not need to be the same length, but they should be. VN's length will be the length of the longest vector in the arguments. :vector_add and :vector_sub will pad out the smaller vectors with 0's or 0.0's. :vector_mul and :vector_div will pad out the smaller vectors with 1's or 1.0's. Vectors do not need to contain homogeneous data, but the nth term of each vector must be of the same type.";
    "I can see a reason for wanting to do vector addition or subtraction, but multiplication and divareion is usually handled in other ways. I've included them here for novelty, and becuase it was easy enough to do.";
    "";
    "Vector addition is used when two or more similar vector quantities are at work and need to be resolved into a single vector. For instance, a ship travelling in a current will be acted upon by (at least) two forces: a force propelling it forward (its engine), and a force pushing it off course (the current). The sum of these two forces gives the resultant net force acting upon the ship and, since Force = Mass * Acceleration, the direction the ship is accelerating.";
    "";
    "Vector subtraction can be used to reverse the process of vector addition. In the ship problem above, let's say the actual resultant force is known, but it does not match the result of adding the propelling force and the drifting force. Friction is probably acting against the motion of the ship. Subtracting the computed resultant force from the known net force will yield the frictional force acting against the progress of the ship.";
    "";
    "Vector multiplication and division do not have RL examples, but vector multiplication of this type makes computing the dot product of two vectors simple.";
    "";
    if (length(args) == 1)
      return args;
    elseif (!args)
      return raise(E_INVARG);
    endif
    type = verb[$ - 2..$];
    lresult = max = length(args[1]);
    results = args[1];
    for n in [2..length(args)]
      $command_utils:suspend_if_needed(0);
      if (type == "add")
        for m in [1..min(lcurr = length(args[n]), lresult)]
          results[m] = results[m] + args[n][m];
          $command_utils:suspend_if_needed(0);
        endfor
        if (lcurr > lresult)
          results[lresult + 1..lcurr] = (args[n])[lresult + 1..lcurr];
        endif
      elseif (type == "sub")
        for m in [1..min(lcurr = length(args[n]), lresult)]
          results[m] = results[m] - args[n][m];
          $command_utils:suspend_if_needed(0);
        endfor
        if (lcurr > lresult)
          for m in [lresult + 1..lcurr]
            results = {@results, -(args[n][m])};
            $command_utils:suspend_if_needed(0);
          endfor
        endif
      elseif (type == "mul")
        for m in [1..min(lcurr = length(args[n]), lresult)]
          results[m] = results[m] * args[n][m];
          $command_utils:suspend_if_needed(0);
        endfor
        if (lcurr > lresult)
          results[lresult + 1..lcurr] = (args[n])[lresult + 1..lcurr];
        endif
      else
        for m in [1..min(lcurr = length(args[n]), lresult)]
          results[m] = results[m] / args[n][m];
          $command_utils:suspend_if_needed(0);
        endfor
        if (lcurr > lresult)
          for m in [lresult + 1..lcurr]
            results = {@results, typeof(foo = args[n][m]) == INT ? 1 / foo | 1.0 / foo};
            $command_utils:suspend_if_needed(0);
          endfor
        endif
      endif
    endfor
    return results;
  endverb

  verb "matrix_add matrix_sub" (this none this) owner: HACKER flags: "rxd"
    ":matrix_add(M1 [, M2 ...]) => MN such that MN[m][n] = M1[m][n] + M2[m][n]...";
    ":matrix_sub(M1 [, M2 ...]) => MN such that MN[m][n] = M1[m][n] - M2[m][n]...";
    "Matrices should all be of the same size.";
    "";
    "Matrix addition and subtraction is simply the addition or subtraction of the vectors contained in the matrices. See 'help $matrix_utils:vector:add' for more help.";
    type = verb[$ - 2..$];
    results = args[1];
    if (typeof(results[1][1]) == LIST)
      for n in [1..length(results)]
        results[n] = this:(verb)(results[n], @$list_utils:slice(args[2..$], n));
      endfor
    else
      for n in [1..length(results)]
        results[n] = this:(("vector_" + type))(results[n], @$list_utils:slice(args[2..$], n));
      endfor
    endif
    return results;
  endverb

  verb transpose (this none this) owner: HACKER flags: "rxd"
    ":transpose(Mmn) => Mnm";
    "Transpose an m by n matrix into an n by m matrix by making the rows in the original the columns in the output.";
    {mat} = args;
    if (!this:is_matrix(mat))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    j = this:dimensions(mat)[2];
    result = {};
    for n in [1..j]
      result = {@result, this:column(mat, n)};
      $command_utils:suspend_if_needed(0);
    endfor
    return result;
  endverb

  verb determinant (this none this) owner: HACKER flags: "rxd"
    ":determinant(M) => NUM the determinant of the matrix.";
    "";
    "There are several properties of a matrix's determinant. Adding or subtracting a row or column from another row or colum of a matrix does not hange the value of its determinant. Multiplying a row or column of a matrix by a single scalar value has the effect of multiplying the matrix's determinant by the same scalar.";
    "";
    "However, the most dramatic use of determinants is in solving linear equations. For example, the solution to this system of equations:";
    "";
    "Ax1 + Bx2 + Cx3 = D";
    "Ex1 + Fx2 + Gx3 = H";
    "Ix1 + Jx2 + Kx3 = L";
    "";
    "is";
    "";
    "     1 |D B C|         1 |A D C|        1 |A B D|";
    "x1 = - |H F G|    x2 = - |E H G|   x3 = - |E F H|";
    "     Z |L J K|         Z |I L K|        Z |I J L|";
    "";
    "          |A B C|";
    "where Z = |E F G|";
    "          |I J K|";
    "";
    "or, in other words, x1, x2, and x3 are some determinant divided by Z, another determinant.";
    "";
    "Determinants are also used in computing the cross product of two vectors. See 'help $matrix_utils:cross_prod' for more info.";
    "";
    {mat} = args;
    if (!this:is_square(mat))
      return raise("E_INVMAT", "Invalid Matrix Format");
    elseif (this:dimensions(mat) == {1, 1})
      return mat[1][1];
    elseif (this:dimensions(mat)[1] == 2)
      return mat[1][1] * mat[2][2] - mat[1][2] * mat[2][1];
    else
      result = typeof(mat[1][1]) == INT ? 0 | 0.0;
      coeff = typeof(mat[1][1]) == INT ? 1 | 1.0;
      for n in [1..length(mat[1])]
        result = result + coeff * mat[1][n] * this:determinant(this:submatrix(1, n, mat));
        coeff = -coeff;
      endfor
      return result;
    endif
    "elseif dims == {1,1} lines are courtesy of Link (#122143).  21-Oct-05";
    "Originated by Uther. Modified by Link (#122143) on 16-Nov-2005.";
  endverb

  verb inverse (this none this) owner: HACKER flags: "rxd"
    ":inverse(M) => MN such that M * MN = I";
    "";
    "The inverse of a matrix is very similar to the reciprocal of a scalar number. If two numbers, A and B, equal 1 (the scalar identity number) when multiplied together (AB=1), then B is said the be the reciprocal of A, and A is the reciprocal of B. If A and B are matrices, and the result of multiplying them togeter is the Identity Matrix, then B is the inverse of A, and A is the inverse of B.";
    "";
    "Computing the inverse involves the solutions of several linear equations. Since linear equations can be easily solved with determinants, this is rather simple. See 'help $matrix_utils:determinant' for more on how determinants solve linear equations.";
    "";
    {mat} = args;
    {i, j} = this:dimensions(mat);
    if (tofloat(det = this:determinant(mat)) == 0.0)
      return raise("E_NOINV", "No Inverse Exists");
    endif
    result = {};
    for k in [1..i]
      sub = {};
      for l in [1..j]
        sub = {@sub, $math_utils:pow(typeof(mat[1][1]) == INT ? -1 | -1.0, k + l) * this:determinant(this:submatrix(l, k, mat)) / det};
      endfor
      result = {@result, sub};
    endfor
    return result;
  endverb

  verb identity (this none this) owner: HACKER flags: "rxd"
    ":identity(INT <size>) => Identity matrix (I) of dimensions <size> by <size>.";
    "All elements of I are 0, except for the diagonal elements which are 1.";
    "";
    "The Identity Matrix has the unique property such that when another matrix is multiplied by it, the other matrix remains unchanged. This is similar to the number 1. a*1 = a. A * I = A, if the dimensions of I and A are the same.";
    "";
    n = args[1];
    result = this:null(n, n);
    for i in [1..n]
      result[i][i] = 1;
    endfor
    return result;
  endverb

  verb null (this none this) owner: HACKER flags: "rxd"
    ":null(INT <size>) => Null matrix (O) of dimensions <size> by <size>.";
    "All elements of O are 0.";
    "";
    "The Null Matrix has the property that is equivalent to the number 0; it reduces the original matrix to itself. a * 0 = 0. A * N = N.";
    "";
    {m, ?n = m} = args;
    result = {};
    for i in [1..m]
      result = {@result, {}};
      for j in [1..n]
        result[i] = {@result[i], 0};
      endfor
    endfor
    return result;
  endverb

  verb is_square (this none this) owner: HACKER flags: "rxd"
    ":is_square(M) => 1 iff dimensions of M are equal to each other.";
    {m} = args;
    return this:is_matrix(m) && this:order(m) == 2 && (dim = this:dimensions(m))[1] == dim[2];
  endverb

  verb is_null (this none this) owner: HACKER flags: "rxd"
    ":is_null(M) => 1 iff M is O.";
    m = length(mat = args[1]);
    if (!this:is_square(mat))
      return 0;
    endif
    for i in [1..m]
      for j in [1..m]
        if (mat[i][j] != 0)
          return 0;
        endif
      endfor
    endfor
    return 1;
  endverb

  verb is_identity (this none this) owner: HACKER flags: "rxd"
    ":is_identity(M) => 1 iff M is I.";
    m = length(mat = args[1]);
    if (!this:is_square(mat))
      return 0;
    endif
    for i in [1..m]
      for j in [1..m]
        if (mat[i][j] != 0 && (i != j ? 1 | mat[i][j] != 1))
          return 0;
        endif
      endfor
    endfor
    return 1;
  endverb

  verb "cross_prod outer_prod vector_prod" (this none this) owner: HACKER flags: "rxd"
    ":cross_prod(V1, V2) => VN, the vector perpendicular to both V1 and V2 with length equal to the area of the parallelogram spanned by V1 and V2, and direction governed by the rule of thumb.";
    "";
    "If A = a1i + a2j + a3k, represented as a list as {a1, a2, a3}";
    "and B = b1i + b2j + b3k, or {b1, b2, b3}, then";
    "";
    "        |i  j  k |";
    "A x B = |a1 a2 a3| = |a2 a3|i - |a1 a3|j + |a1 a2|k";
    "        |b1 b2 b3| = |b2 b3|    |b1 b3|    |b1 b2|";
    "";
    "or, in list terms, as the list of the coefficients of i, j, and k.";
    "";
    "Note: i, j, and k are unit vectors in the x, y, and z direction respectively.";
    "";
    "The rule of thumb: A x B = C  If you hold your right hand out so that your fingers point in the direction of A, and so that you can curl them through B as you make a hitchhiking fist, your thumb will point in the direction of C.";
    "";
    "Put another way, A x B = ABsin(THETA) (A cross B equals the magnitude of A times the magnitude of B times the sin of the angle between them) This is expressed as a vector perpendicular the the A-B plane, pointing `up' if you curl your right hand fingers from A to B, and `down' if your right hand fingers curl from B to A.";
    "";
    "The cross product has many uses in physics. Angular momentum is the cross product of a particles position vector from the point it is rotating around and it's linear momentum (L = r x p). Torque is the cross product of position and Force (t = r x F).";
    "";
    {v1, v2} = args;
    if ((l = length(v1)) != length(v2) || l != 3 || !this:is_vector(v1) || !this:is_vector(v2))
      return raise("E_INVVEC", "Invalid Vector Format");
    endif
    mat = {{1, 1, 1}, v1, v2};
    coeff = 1;
    result = {};
    for n in [1..3]
      result = {@result, coeff * this:determinant(this:submatrix(1, n, mat))};
      coeff = -coeff;
    endfor
    return result;
  endverb

  verb "norm length" (this none this) owner: HACKER flags: "rxd"
    ":norm(V) => FLOAT";
    ":length(V) => FLOAT";
    "The norm is the length of a vector, the square root of the sum of the squares of its elements.";
    "";
    "In school, we all should have learned the Pythagorean Theorem of right triangles: The sum of the squares of the sides of a right triagle is equal to the square of the hypoteneuse. The Theorem holds true no matter how many dimensions are being considered. The length of a vector is equal to the square root of the sum of the squares of its components. The dot product of a vector with itself happens to be the sum of the squares of its components.";
    "";
    {v} = args;
    return this:is_vector(v) ? sqrt(tofloat(this:dot_prod(v, v))) | E_TYPE;
  endverb

  verb submatrix (this none this) owner: HACKER flags: "rxd"
    ":submatrix(i, j, M1) => M2, the matrix formed from deleting the ith row and jth column from M1.";
    {i, j, mat} = args;
    {k, l} = this:dimensions(mat);
    result = {};
    for m in [1..k]
      sub = {};
      for n in [1..l]
        if (m != i && n != j)
          sub = {@sub, mat[m][n]};
        endif
      endfor
      if (sub)
        result = {@result, sub};
      endif
    endfor
    return result;
  endverb

  verb "dot_prod inner_prod scalar_prod" (this none this) owner: HACKER flags: "rxd"
    ":dot_prod(V1, V2) => NUM";
    ":inner_prod(V1, V2) => NUM";
    "The dot, or inner, product of two vectors is the sum of the products of the corresponding elements of the vectors.";
    "If V1 = {1, 2, 3} and V2 = {4, 5, 6}, then V1.V2 = 1*4 + 2*5 + 3*6 = 32";
    "";
    "The dot product is useful in computing the angle between two vectors, and the length of a vector. See 'help $matrix_utils:subtended_angle' and 'help $matrix_utils:length'.";
    "";
    "A . B = ABcos(THETA)  (A dot B equals the magnitude of A times the magnitude of B times the cosine of the angle between them.)";
    "";
    {v1, v2} = args;
    if ((l = length(v1)) != length(v2) || !this:is_vector(v1) || !this:is_vector(v2))
      return raise("E_INVVEC", "Invalid Vector Format");
    endif
    temp = this:vector_mul(v1, v2);
    result = typeof(temp[1]) == INT ? 0 | 0.0;
    for n in [1..l]
      $command_utils:suspend_if_needed(0);
      result = result + temp[n];
    endfor
    return result;
  endverb

  verb "dimension*s" (this none this) owner: HACKER flags: "rxd"
    ":dimensions(M) => LIST of dimensional sizes.";
    l = {length(m = args[1])};
    if (typeof(m[1]) == LIST)
      l = {@l, @this:dimensions(m[1])};
    endif
    return l;
  endverb

  verb order (this none this) owner: HACKER flags: "rxd"
    ":order(M) => INT how many dimensions does this matrix have? 1 means vector";
    return length(this:dimensions(args[1]));
  endverb

  verb "scalar_vector_add scalar_vector_sub scalar_vector_mul scalar_vector_div" (this none this) owner: HACKER flags: "rxd"
    ":scalar_vector_add(S, V) => VN such that VN[n] = V[n] + S...";
    ":scalar_vector_sub(S, V) => VN such that VN[n] = V[n] - S...";
    ":scalar_vector_mul(S, V) => VN such that VN[n] = V[n] * S...";
    ":scalar_vector_div(S, V) => VN such that VN[n] = V[n] / S...";
    "Actually, arguments can be (S, V) or (V, S). Each element of V is augmented by S. S should be either an INT or a FLOAT, as appropriate to the values in V.";
    "";
    "I can see a reason for wanting to do scalar/vector multiplcation or division, but addition and subtraction between vector and scalar types is not done. I've included them here for novelty, and because it was easy enough to to.";
    "";
    "Scalar-vector multiplication stretches a vector along its direction, generating points along a line. One of the more famous uses from physics is Force equals mass times acceleration. F = ma. Force and acceleration are both vectors. Mass is a scalar quantity.";
    "";
    if (typeof(args[1]) == LIST)
      {vval, sval} = args;
    else
      {sval, vval} = args;
    endif
    if (!this:is_vector(vval))
      return raise("E_INVVEC", "Invalid Vector Format");
    endif
    type = verb[$ - 2..$];
    for n in [1..length(vval)]
      if (type == "add")
        vval[n] = vval[n] + sval;
      elseif (type == "sub")
        vval[n] = vval[n] - sval;
      elseif (type == "mul")
        vval[n] = vval[n] * sval;
      else
        vval[n] = vval[n] / sval;
      endif
    endfor
    return vval;
  endverb

  verb subtended_angle (this none this) owner: HACKER flags: "rxd"
    ":subtended_angle(V1, V2) => FLOAT smallest angle defined by V1, V2 in radians";
    "";
    "Any two vectors define two angles, one less than or equal to 180 degrees, the other 180 degrees or more. The larger can be determined from the smaller, since their sum must be 360 degrees.";
    "";
    "The dot product of the two angles, divided by the lengths of each of the vectors is the cosine of the smaller angle defined by the two vectors.";
    "";
    {v1, v2} = args;
    if ((l = length(v1)) != length(v2) || !this:is_vector(v1) || !this:is_vector(v2))
      return raise("E_INVVEC", "Invalid Vector Format");
    endif
    return acos(tofloat(this:dot_prod(v1, v2)) / (this:norm(v1) * this:norm(v2)));
  endverb

  verb column (this none this) owner: HACKER flags: "rxd"
    ":column(M, INT <n>) => LIST the nth column of M.";
    {mat, i} = args;
    j = this:dimensions(mat)[1];
    result = {};
    for m in [1..j]
      result = {@result, mat[m][i]};
      $command_utils:suspend_if_needed(0);
    endfor
    return result;
  endverb

  verb matrix_mul (this none this) owner: HACKER flags: "rxd"
    ":matrix_mul(M1, M2) => MN such that MN[m][n] = the dot product of the mth row of M1 and the transpose of thenth column of M2.";
    "";
    "Matrix multiplication is the most common and complex operation performed on two matrices. First, matrices can only be multiplied if they are of compatible sizes. An i by j matrix can only be multiplied by a j by k matrix, and the results of this multiplication will be a matrix of size i by k. Each element in the resulting matrix is the dot product of a row from the first matrix and a column from the second matrix. (See 'help $matrix_utils:dot_prod'.)";
    "";
    {m1, m2} = args;
    {i, j} = this:dimensions(m1);
    {k, l} = this:dimensions(m2);
    if (j != k || !this:is_matrix(m1) || !this:is_matrix(m2))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    result = {};
    for m in [1..i]
      sub = {};
      for n in [1..l]
        $command_utils:suspend_if_needed(0);
        sub = {@sub, this:dot_prod(m1[m], this:column(m2, n))};
      endfor
      result = {@result, sub};
    endfor
    return result;
  endverb

  verb "scalar_matrix_mul scalar_matrix_div" (this none this) owner: HACKER flags: "rxd"
    ":scalar_matrix_add(S, M) => MN such that MN[m][n] = MN[m][n] + S...";
    ":scalar_matrix_sub(S, M) => MN such that MN[m][n] = MN[m][n] - S...";
    ":scalar_matrix_mul(S, M) => MN such that MN[m][n] = MN[m][n] * S...";
    ":scalar_matrix_div(S, M) => MN such that MN[m][n] = MN[m][n] / S...";
    "Actually, arguments can be (S, M) or (M, S). Each element of M is augmented by S. S should be either an INT or a FLOAT, as appropriate to the values in M.";
    "I can see a reason for wanting to do scalar/matrix multiplication or division, but addition and subtraction between matrix and scalar types is not done. I've included them here for novelty, and because it was easy enough to do.";
    type = verb[$ - 2..$];
    if (typeof(args[1]) == LIST)
      {mval, sval} = args;
    else
      {sval, mval} = args;
    endif
    if (!this:is_matrix(mval))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    results = {};
    if (typeof(mval[1][1] == LIST))
      for n in [1..length(mval)]
        results = {@results, this:(verb)(mval[n], sval)};
      endfor
    else
      for n in [1..length(mval)]
        results = {@results, this:(("scalar_vector_" + type))(mval[n], sval)};
      endfor
    endif
    return results;
  endverb

  verb is_matrix (this none this) owner: HACKER flags: "rxd"
    "A matrix is defined as a list of vectors, each having the smae number of elements.";
    {m} = args;
    if (typeof(m) != LIST || typeof(m[1]) != LIST)
      return 0;
    endif
    len = length(m[1]);
    for v in (m)
      if (!this:is_vector(v) || length(v) != len)
        return 0;
      endif
    endfor
    return 1;
  endverb

  verb is_vector (this none this) owner: HACKER flags: "rxd"
    "A vector shall be defined as a list of INTs or FLOATs. (I'm not gonna worry about them all being the same type.)";
    flag = 1;
    {v} = args;
    if (typeof(v) != LIST)
      return 0;
    endif
    for n in (v)
      if ((ntype = typeof(n)) != INT && ntype != FLOAT)
        flag = 0;
        break;
      endif
      $command_utils:suspend_if_needed(0);
    endfor
    return flag;
  endverb

  verb "is_reflexive is_areflexive" (this none this) owner: HACKER flags: "rxd"
    ":is_reflexive   (M) => 1 if M is a reflexive relation, -1 if areflexive,";
    "                       0 otherwise.";
    ":is_areflexive does the same, but with 1 and -1 reversed.";
    {m} = args;
    if (!this:is_square(m))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    good = bad = 0;
    for n in [1..length(m)]
      if (!(m[n][n]))
        bad = 1;
      else
        good = 1;
      endif
    endfor
    return this:_relation_result(good, bad, verb[4] == "a");
  endverb

  verb "is_symmetric is_asymmetric" (this none this) owner: HACKER flags: "rxd"
    ":is_symmetric   (M) => 1 if M is a symmetric relation, -1 if asymmetric,";
    "                       0 otherwise.";
    ":is_asymmetric does the same, but with 1 and -1 reversed.";
    {mat} = args;
    if (!this:is_square(mat))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    good = bad = 0;
    for m in [1..len = length(mat)]
      for n in [m + 1..len]
        if (mat[m][n] == mat[n][m])
          good = 1;
        else
          bad = 1;
        endif
      endfor
    endfor
    return this:_relation_result(good, bad, verb[4] == "a");
  endverb

  verb "is_transitive is_atransitive" (this none this) owner: HACKER flags: "rxd"
    ":is_transitive  (M) => 1 if M is a transitive relation, -1 if atransitive,";
    "                       0 otherwise.";
    ":is_atransitive does the same, but with 1 and -1 reversed.";
    {mat} = args;
    if (!this:is_square(mat))
      return raise("E_INVMAT", "Invalid Matrix Format");
    endif
    good = bad = 0;
    for m in [1..len = length(mat)]
      for n in [1..len]
        if (mat[m][n])
          for l in [1..len]
            if (mat[n][l])
              if (mat[m][l])
                good = 1;
              else
                bad = 1;
              endif
            endif
          endfor
        endif
      endfor
    endfor
    return this:_relation_result(good, bad, verb[4] == "a");
  endverb

  verb _relation_result (this none this) owner: HACKER flags: "rxd"
    "Common code for is_reflexive, is_symmetric, and is_transitive.";
    {good, bad, flag} = args;
    if (good && !bad)
      result = 1;
    elseif (!good && bad)
      result = -1;
    else
      result = 0;
    endif
    return flag * result;
  endverb

  verb is_partial_ordering (this none this) owner: HACKER flags: "rxd"
    ":is_partial_ordering(M) => 1 iff M is a reflexive, asymmetric, transitive relation.";
    {mat} = args;
    return this:is_asymmetric(mat) == this:is_reflexive(mat) == this:is_transitive(mat) == 1;
  endverb
endobject