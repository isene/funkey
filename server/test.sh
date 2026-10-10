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
want "a game not on the list"                 400 "Which game?|"     "$(req GET game=raid 1.1.1.1 '')"
want "a path as the game"                     400 "Which game?|"     "$(req GET 'game=../../etc/passwd' 1.1.1.1 '')"
want "a game name with more behind it"        400 "Which game?|"     "$(req GET game=salvox 1.1.1.1 '')"
want "no game"                                400 "Which game?|"     "$(req GET '' 1.1.1.1 '')"
want "another method"                         405 "GET or POST.|"    "$(req DELETE game=salvo 1.1.1.1 '')"
for n in 1 2 3 4 5 6 7 8 9 10 11; do req POST game=gems 2.2.2.$n "AAA $((n * 10))" > /dev/null; done
want "ten are kept, the lowest falls off"     200 "AAA 110|AAA 100|AAA 90|AAA 80|AAA 70|AAA 60|AAA 50|AAA 40|AAA 30|AAA 20|" "$(req GET game=gems 1.1.1.1 '')"
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

[ $fail = 0 ] && echo "ALL OK" || { echo "SOME FAILED"; exit 1; }
