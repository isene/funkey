#!/bin/bash
# Tests for scores.rb: the script is run as the web server runs it, one
# request a call, against lists in a scratch folder. Needs ruby.
#
#   server/test.sh
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
mkdir "$tmp/lists"
fail=0

# method, query, address, body: the script's whole answer.
req() {
  printf '%s' "$4" | env -i PATH=/usr/bin:/bin ${SCORES:+FUNKEY_SCORES="$SCORES"} REQUEST_METHOD="$1" QUERY_STRING="$2" \
    REMOTE_ADDR="$3" CONTENT_LENGTH="${#4}" ruby "${SCRIPT:-$here/scores.rb}" 2>/dev/null | tr -d '\r'
}

# what, the status wanted, the body wanted (a | for each line end), the answer.
want() {
  local status body
  status=$(printf '%s\n' "$4" | head -1)
  body=$(printf '%s\n' "$4" | sed '1,/^$/d' | tr '\n' '|')
  if [[ "$status" == "Status: $2"* && "$body" == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: [$status] [$body]"; fail=1; fi
}

SCORES="$tmp/lists"
want "an empty list"                          200 ""                 "$(req GET game=salvo 1.1.1.1 '')"
want "a score goes on the list"               200 "GEI 500|"         "$(req POST game=salvo 1.1.1.2 'GEI 500')"
want "a second one, best first"               200 "TOP 900|GEI 500|" "$(req POST game=salvo 1.1.1.3 'TOP 900')"
want "the list is read back"                  200 "TOP 900|GEI 500|" "$(req GET game=salvo 1.1.1.1 '')"
want "another game has its own list"          200 ""                 "$(req GET game=vector 1.1.1.1 '')"
want "too soon from one address"              429 "One score every 15 seconds.|" "$(req POST game=vector 1.1.1.3 'TOP 900')"
want "stack's four fields are no salvo score" 400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.1.4 'GEI 500 40 5')"
want "two fields are no stack score"          400 "Send: ABC 12345 40 5|" "$(req POST game=stack 1.1.1.5 'GEI 500')"
want "stack with its rows and level"          200 "GEI 500 40 5|"    "$(req POST game=stack 1.1.1.6 'GEI 500 40 5')"
want "stack checks what is possible"          400 "The game cannot give that score.|" "$(req POST game=stack 1.1.1.7 'GEI 9999999 1 1')"
want "no score of nothing"                    400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.1.8 'GEI 0')"
want "small letters"                          400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.1.9 'gei 500')"
want "eight digits"                           400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.2.1 'GEI 12345678')"
want "a second line"                          400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.2.2 $'GEI 5\nBAD 9')"
want "shell text"                             400 "Send: ABC 12345|" "$(req POST game=salvo 1.1.2.3 'GEI 5;id')"
want "too long"                               413 "Too long.|"       "$(req POST game=salvo 1.1.2.4 "$(printf 'A%.0s' {1..70})")"
want "a game not on the list"                 400 "Which game?|"     "$(req GET game=doom 1.1.1.1 '')"
want "a path as the game"                     400 "Which game?|"     "$(req GET 'game=../../etc/passwd' 1.1.1.1 '')"
want "a game name with more behind it"        400 "Which game?|"     "$(req GET game=salvox 1.1.1.1 '')"
want "no game"                                400 "Which game?|"     "$(req GET '' 1.1.1.1 '')"
want "another method"                         405 "GET or POST.|"    "$(req DELETE game=salvo 1.1.1.1 '')"
for n in 1 2 3 4 5 6 7 8 9 10 11; do req POST game=gems 2.2.2.$n "AAA $((n * 10))" > /dev/null; done
want "ten are kept, the lowest falls off"     200 "AAA 110|AAA 100|AAA 90|AAA 80|AAA 70|AAA 60|AAA 50|AAA 40|AAA 30|AAA 20|" "$(req GET game=gems 1.1.1.1 '')"
want "the best of each list"                  200 "stack GEI 500|gems AAA 110|salvo TOP 900|" "$(req GET game=all 1.1.1.1 '')"
want "no score for all"                       400 "Which game?|"     "$(req POST game=all 1.1.3.1 'GEI 500')"
printf 'junk\n<script>\n' >> "$tmp/lists/salvo.txt"
want "junk in a file is not sent"             200 "TOP 900|GEI 500|" "$(req GET game=salvo 1.1.1.1 '')"
[[ "$(cd "$tmp/lists" && echo *)" == "gems.txt recent.txt salvo.txt stack.txt" ]] && echo "ok   only the lists and recent.txt are written" || { echo "FAIL files: $(cd "$tmp/lists" && echo *)"; fail=1; }

# As on the server: a checkout, the lists beside it, a link in cgi-bin,
# no FUNKEY_SCORES, and a home folder that is somewhere else.
mkdir -p "$tmp/home/funkey/server" "$tmp/home/funkey-scores" "$tmp/www/cgi-bin"
cp "$here/scores.rb" "$tmp/home/funkey/server/scores.rb"
ln -s "$tmp/home/funkey/server/scores.rb" "$tmp/www/cgi-bin/funkey-scores.rb"
echo "ABC 5" > "$tmp/home/funkey-scores/salvo.txt"
SCORES="" SCRIPT="$tmp/www/cgi-bin/funkey-scores.rb"
want "the lists beside the checkout are found through the link" 200 "ABC 5|" "$(req GET game=salvo 1.1.1.1 '')"


# comments.rb, the same way: method, query, address, body. The time in
# each comment is set to 0, so answers compare.
mkdir "$tmp/c"
creq() {
  printf '%s' "$4" | env -i PATH=/usr/bin:/bin FUNKEY_SCORES="${CDIR:-$tmp/c}" REQUEST_METHOD="$1" QUERY_STRING="$2" \
    REMOTE_ADDR="$3" CONTENT_LENGTH="$(printf '%s' "$4" | wc -c)" ruby "${CSCRIPT:-$here/comments.rb}" 2>/dev/null \
    | tr -d '\r' | sed 's/"t":[0-9]*/"t":0/g'
}
A='{"t":0,"n":"Alice","c":"Fun game!"}'
want "no comments yet"                        200 "[]|"              "$(creq GET game=wargame 3.3.3.1 '')"
want "a comment shows at once"                200 "[$A]|"            "$(creq POST game=wargame 3.3.3.2 $'Alice\nFun game!')"
want "it is read back"                        200 "[$A]|"            "$(creq GET game=wargame 3.3.3.1 '')"
want "too soon from one address"              429 "One comment every 30 seconds.|" "$(creq POST game=wargame 3.3.3.2 $'Alice\nAgain')"
want "no name is a player"                    200 "[$A,{\"t\":0,\"n\":\"A player\",\"c\":\"Hi\"}]|" "$(creq POST game=wargame 3.3.3.3 $'\nHi')"
want "lines, letters and marks as typed"      200 "[{\"t\":0,\"n\":\"Bjørn\",\"c\":\"Første!\\n\\n<b>to</b> 😀\"}]|" \
  "$(creq POST game=salvo 3.3.3.4 $'Bjørn\nFørste!\n\n\n\n<b>to</b> 😀')"
want "control marks and right-to-left go"     200 "[{\"t\":0,\"n\":\"Eve x\",\"c\":\"ab\"}]|" \
  "$(creq POST game=stack 3.3.3.5 $'Eve\tx\na‮b\x07')"
want "nothing written"                        400 "Write something.|" "$(creq POST game=wargame 3.3.3.6 $'Bob\n  \n ')"
want "a long name"                            400 "A name of at most 30 letters.|" "$(creq POST game=wargame 3.3.3.7 "$(printf 'B%.0s' {1..31})"$'\nHi')"
want "a long text"                            400 "At most 1000 letters.|" "$(creq POST game=wargame 3.3.3.8 $'Bob\n'"$(printf 'x%.0s' {1..1001})")"
want "a body far too long"                    413 "Too long.|"       "$(creq POST game=wargame 3.3.3.9 "$(printf 'x%.0s' {1..4200})")"
want "not UTF-8"                              400 "Send text as UTF-8.|" "$(creq POST game=wargame 3.3.4.1 $'Bob\n\xff\xfe')"
want "a game not in the room"                 400 "Which game?|"     "$(creq GET game=gems 3.3.3.1 '')"
want "a path as the game"                     400 "Which game?|"     "$(creq GET 'game=../../etc/passwd' 3.3.3.1 '')"
want "a game name with more behind it"        400 "Which game?|"     "$(creq GET 'game=salvo.txt' 3.3.3.1 '')"
want "no comment on all"                      400 "Which game?|"     "$(creq POST game=all 3.3.4.2 $'Bob\nHi')"
want "a comment count for each game"        200 '{"raid":0,"kart":0,"salvo":1,"wargame":2,"stack":1,"again":0,"marble":0,"chomp":0,"eliminator":0,"vector":0,"blocks":0,"jumpman":0}|' \
  "$(creq GET game=all 3.3.3.1 '' | sed 's/^{"counts":\({[^}]*}\).*/\1/')"
want "another method"                         405 "GET or POST.|"    "$(creq PUT game=wargame 3.3.3.1 '')"
printf 'junk\n{"t":"x","n":1}\n<script>\n' >> "$tmp/c/comments-wargame.txt"
want "junk in a file is not sent"             200 "[$A,{\"t\":0,\"n\":\"A player\",\"c\":\"Hi\"}]|" "$(creq GET game=wargame 3.3.3.1 '')"
[[ "$(cd "$tmp/c" && echo *)" == "comments-salvo.txt comments-stack.txt comments-wargame.txt recent-comments.txt" ]] \
  && echo "ok   only the comment files and recent-comments.txt are written" || { echo "FAIL files: $(cd "$tmp/c" && echo *)"; fail=1; }
mkdir "$tmp/c2"
printf '%s\n' '{"t":1,"n":"Ann","c":"Old"}' '{"t":3,"n":"Bob","c":"New"}' > "$tmp/c2/comments-raid.txt"
printf '%s\n' '{"t":2,"n":"Cy","c":"Mid"}' > "$tmp/c2/comments-kart.txt"
want "the newest comments of all, newest first" 200 '[{"t":0,"n":"Bob","c":"New","g":"raid"},{"t":0,"n":"Cy","c":"Mid","g":"kart"},{"t":0,"n":"Ann","c":"Old","g":"raid"}]|' \
  "$(CDIR="$tmp/c2" creq GET game=all 3.3.3.1 '' | sed 's/.*"latest"://; s/}$//')"
cp "$here/comments.rb" "$tmp/home/funkey/server/comments.rb"
ln -s "$tmp/home/funkey/server/comments.rb" "$tmp/www/cgi-bin/funkey-comments.rb"
printf '%s\n' '{"t":1,"n":"Ann","c":"Hi"}' > "$tmp/home/funkey-scores/comments-raid.txt"
cget() { env -i PATH=/usr/bin:/bin REQUEST_METHOD=GET QUERY_STRING="$1" REMOTE_ADDR=3.3.3.1 ruby "$tmp/www/cgi-bin/funkey-comments.rb" 2>/dev/null | tr -d '\r'; }
want "comments beside the checkout are found through the link" 200 '[{"t":1,"n":"Ann","c":"Hi"}]|' "$(cget game=raid)"

# The web server runs a script itself, so it must be marked to run.
for s in scores.rb comments.rb; do
  [ -x "$here/$s" ] && echo "ok   $s may run" || { echo "FAIL $s is not marked to run"; fail=1; }
done

[ $fail = 0 ] && echo "ALL OK" || { echo "SOME FAILED"; exit 1; }
