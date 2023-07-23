Copy/fork of and Rust bindings for the LambdaMOO regexp implementation, which
itself is a fork of the old Python <1.3 regex implementation. There are various
differences between modern regex and this implementation, so rather than try to
rewrite the regex engine from scratch, and debug all the edge cases to get 100%
compatibility for existing cores, I'm just going to wrap this.

I will provide a separate builtin that uses modern regexp matching, and new code
should use that. This is just for compatibility with existing cores.



