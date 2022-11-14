grammar moo;

// Parser
program: statements EOF;

statements: statement*;

statement:
        'if' '(' expr ')' statements elseifs ('else' elsepart=statements)? 'endif' #If
    |   'for' ID 'in' '(' expr ')' statements 'endfor' #ForExpr
    |   'for' ID 'in' '[' from=expr TO to=expr ']' statements 'endfor' #ForRange
    |   'while' ID? '(' condition=expr ')' statements 'endwhile' #While
    |   'fork' ID? '(' time=expr ')' statements 'endfork' #Fork
    |   'break' ID? ';' #Break
    |   'continue' ID? #Continue
    |   'return' expr? ';' #Return
    |   'try' statements excepts 'endtry' #TryExcept
    |   'try' statements 'finally' statements 'endtry' #TryFinally
    |   expr? ';' #ExprStmt
    ;

elseifs: /* */ |
    elseifs 'elseif' '(' expr ')' statements
    ;

excepts: except+;
except: 'except' (id=ID?) '(' codes ')' statements;

literal:
        INTEGER #Int
    |   FLOAT #Float
    |   STRING #String
    |   OBJECT #Object
    |   ERROR #Error
    |   ID #Identifier;


expr:
       '$' #RangeEnd
    |   '{' scatter '}' '=' expr #ScatterEXpr
    |   '{' arglist  '}' #ListExpr
    |   expr '[' expr TO expr ']' #IndexRangeExpr
    |   expr '[' expr ']' #IndexExpr
    |   <assoc=right> expr '=' expr #AssigneXpr
    |   expr '+' expr #AddExpr
    |   expr '-' expr #SubExpr
    |   expr '*' expr #MulExpr
    |   expr '/' expr #DivExpr
    |   expr '%' expr #ModExpr
    |   expr '^' expr #XorExpr
    |   expr '&&' expr #AndExpr
    |   expr '||' expr #OrExpr
    |   expr '==' expr #EqExpr
    |   expr '!=' expr #NeExpr
    |   expr '<' expr #LtExpr
    |   expr '<=' expr #LtEExpr
    |   expr '>' expr #GtExpr
    |   expr '>=' expr #GtEExpr
    |   expr 'in' expr #InExpr
    |   expr '=>' expr #ArrowExpr
    |   '-' expr #NegateExpr
    |   '!' expr #NotExpr
    |   '(' expr ')' #ParenExpr
    |   expr '?' expr '|' expr #IfExpr
    |  '`' expr '!' codes default_br '\'' #ErrorEscape
    |  literal #LiteralExpr
    |  '$'? id=ID '(' arglist ')' #SysVerb
    |  location=expr ':' '(' verb=expr ')' '(' arglist ')' # VerbExprCall
    |  location=expr ':' verb=ID '(' arglist ')' #VerbCall
    |  '$'? id=ID #SysProp
    |  location=expr '.' property=ID  #PropertyReference
    |  location=expr '.' '(' property=expr ')' #PropertyExprReference
    ;

codes:
    'any' | ne_arglist;

default_br:
    /* nothing */ | ne_arglist;

arglist: /* emmpty*/ |
        ne_arglist;

ne_arglist:
        argument (',' argument)*
      ;
argument: expr #Arg | '@' expr #SpliceArg;

scatter:
        ne_arglist ',' scatter_item
    |   scatter ',' scatter_item
    |   scatter ',' ID
    |   scatter ',' '@' ID
    |   scatter_item
;

scatter_item:
        '?' ID
    |   '?' ID '=' expr;


// Lexer
Whitespace
    :   [ \t]+
        -> skip
    ;

Newline
    :   (   '\r' '\n'?
        |   '\n'
        )
        -> skip
    ;

ID: [a-zA-Z_][a-zA-Z_0-9]*;
STRING:  '"' ( EscapeSequence | ~('\\'|'"') )* '"' ;
INTEGER: (Sign)? INT | HEX ;
INT: Digit+;
HEX
    : '0' [xX] HexDigit+
    ;

FLOAT
    : Digit+ '.' Digit+ ExponentPart?
    | '.' Digit+ ExponentPart?
    | Digit+ ExponentPart
    ;

OBJECT: '#' INTEGER;

DOT: '.';
TO: '..';

fragment
Sign
    :   [+-]
    ;

fragment IF: 'if';

fragment
ExponentPart
    : [eE] [+-]? Digit+
    ;

fragment
HexExponentPart
    : [pP] [+-]? Digit+
    ;

fragment
EscapeSequence
    : '\\' [abfnrtvz"'\\]
    | '\\' '\r'? '\n'
    | DecimalEscape
    | HexEscape
    ;


fragment
DecimalEscape
    : '\\' Digit
    | '\\' Digit Digit
    | '\\' [0-2] Digit Digit
    ;

fragment
HexEscape
    : '\\' 'x' HexDigit HexDigit
    ;

fragment
Digit
    : [0-9]
    ;
fragment
HexDigit
    : [0-9a-fA-F]
    ;


ERROR: 'e_type' | 'e_div' | 'e_perm' | 'e_propnf' | 'e_verbnf' | 'e_varnf' | 'e_invind' | 'e_recmove' |
       'e_maxrec' | 'e_range' | 'e_args' | 'e_nacc' | 'e_invarg' | 'e_quota' | 'e_float';
