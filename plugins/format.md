# commands
- /confirm <...message...> -> literal 1 or 0
- /ask <...message...> -> string
- /info <...message...>
- /warn <...message...>
- /error <...message...>
- /result -> see # result format

# result format
`key` `lines`
`lines of content`

## note
After using /result, the following content you print will belong to result.
Available `key`s: limit, input, answer
`lines` is positive integer

## template
limit 2
memory 5242880
time 1000
input 1
1 1
answer 1
2
input 3
50

50
answer 1
100
