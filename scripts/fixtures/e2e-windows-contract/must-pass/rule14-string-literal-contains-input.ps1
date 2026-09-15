function Get-ExampleSnippet {
    <#
    .SYNOPSIS
        Documents the historical bug in prose that literally contains the
        banned shape as text, never as code - the AST walk must not be
        fooled by a string constant that happens to spell '$input ='.
    #>
    return 'the bug looked like this: $input = Require-Target ...'
}
