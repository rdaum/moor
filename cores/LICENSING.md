# Licensing Notice for MOO Core Databases

This directory contains MOO core databases that are distributed under separate licensing terms
from the main mooR project.

## Core Database Files

### lambda-moor/

- **Source**: LambdaMOO core adapted for mooR ; derived from LambdaCore extraction from LambdaMOO:
  See https://lisdude.com/moo/#cores
- **Original Author**: Pavel Curtis and the LambdaMOO community + modifications by the mooR authors
- **License Status**: Complex/Unclear - see notes below
- **Notes**: Standard LambdaMOO core database with community contributions
- **Notes**: LambdaCore adaptation for mooR compatibility
- **Original LambdaCore Location**: The original official LambdaCore releases previously at parcftp.xerox.com are now available at https://lambda.moo.mud.org/pub/MOO/

### JHCore-DEV-2.db

- **Source**: JHCore development database (originally derived from LambdaCore)
- **Original Author**: Various MOO community contributors especially from Waterpoint MOO
- **License Status**: Mixed - JHCore authors' modifications are explicitly licensed, but LambdaCore-derived portions
  remain complex
- **Notes**: Community-developed core with additional features
- **Original LambdaCore Location**: The original official LambdaCore releases previously at parcftp.xerox.com are now available at https://lambda.moo.mud.org/pub/MOO/

#### JHCore License

**Important Note**: This license applies to the modifications and new works created by the JHCore authors. The
underlying LambdaCore-derived portions still have the complex licensing situation described below.

```
CORE-LICENSE
============
Portions of this database are derived from the LambdaCore
distribution, originally available for anonymous ftp at parcftp.xerox.com
(now available at https://lambda.moo.mud.org/pub/MOO/).  The
following copyright notice applies to new and derived works within
this database.

Copyright 1991, 1992, 1993, 1994, 1995, 1996, 1998, 2001, 2002 by Ken Fox.
                        All Rights Reserved

Permission to use, copy, modify, and distribute this software and its
documentation for any purpose and without fee is hereby granted,
provided that the above copyright notice appear in all copies and that
both that copyright notice and this permission notice appear in
supporting documentation.

KEN FOX DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS SOFTWARE, INCLUDING
ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS, IN NO EVENT SHALL
KEN FOX BE LIABLE FOR ANY SPECIAL, INDIRECT OR CONSEQUENTIAL DAMAGES OR
ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THIS
SOFTWARE.
```

### minimal-core/

- **Source**: Minimal MOO core for testing
- **License**: GPL-3.0 (same as main mooR project)
- **Notes**: This is the only core database covered by mooR's GPL-3.0 license

## Important Licensing Notes

### LambdaCore and Derivatives (JHCore-DEV-2.db, lambda-moor/)

**The licensing situation for LambdaCore and its derivatives is complex:**

We are not lawyers, but here's our best summation of the current situation.

1. **No Explicit License**: LambdaCore was distributed without an explicit license
2. **Multiple Contributors**: Contains contributions from many authors with no unified licensing
3. **No Copyright Assertion**: Original author Pavel Curtis stated neither he nor Xerox "assert any copyright over any
   part of the LambdaCore code" (1994)
4. **Implied License**: The long-standing distribution practice suggests an implied license for use

## Historical Reference: Pavel Curtis's Email (1994)

```
Date:    Sat, 27 Aug 1994 15:35:49 -0700
To:    scohn@nyx10.cs.du.edu (Seth Cohn), moo-cows@parc.xerox.com
From:    pavel@parc.xerox.com (Pavel Curtis)
Subject: Re: More on moo code copyright
Message-Id: <94Aug27.143148pdt.58378@mu.parc.xerox.com>

At  3:55 PM 8/26/94 -0700, Seth Cohn wrote:
>Not to argue with your lawyers, but I think they are wrong.  That would mean
>that all of the current core code is PD.  Since I think Haakon and Xerox
>would have a thing to say about that, the code must be covered by the
>prevailing overall copyright.

Just FYI, neither Xerox nor I currently assert any copyright over any part
of the LambdaCore code and I don't anticipate anything like that in the
future.  In particular, quite a lot of the core code was written by people
who are neither Xerox employees or signers of any agreement with Xerox.

>MY moo will have the default be "Copyright, but copy,usage,derivative allowed"
>Why?  Because that is my understand of the Xerox copyright that Pavel releases
>the code under.

Please note that the copyright on the server code is unrelated to any
possible copyright on the core.

        Pavel
```

**mooR Project Position:**

- We do NOT claim ownership or copyright over LambdaCore or its derivatives
- We distribute these cores under the same implied license as historical distribution
- We make NO explicit licensing claims about these databases
- Caveat emptor: Users should be aware of the complex licensing situation

### Usage Guidelines

- **JHCore**: Has explicit permission from Ken Fox for use, modification, and distribution
- **lambda-moor**: Use at your own risk with understanding of complex licensing
- **minimal-core**: Covered by mooR's GPL-3.0 license - safe for GPL-compatible use

- These core databases are provided "as-is" for compatibility testing and reference
- For production use, consider using minimal-core or creating your own core database
- The mooR project maintainers are not responsible for licensing compliance of third-party databases

---

**Disclaimer**: This licensing notice represents our best understanding of the complex
historical licensing situation. Users should conduct their own due diligence and consult
legal counsel if concerned about licensing compliance.