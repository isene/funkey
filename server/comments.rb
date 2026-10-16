#!/usr/bin/env ruby
# funkey comments: what players write under each game in the game room at
# isene.com/games. A CGI script like scores.rb, beside it in the checkout,
# linked from cgi-bin/funkey-comments.rb.
#
#   GET  funkey-comments.rb?game=salvo   the comments, oldest first, as JSON:
#                                        [{"t": 1760000000, "n": "Alice", "c": "Fun!"}]
#   GET  funkey-comments.rb?game=all     how many each game has, and the eight
#                                        newest of all with their game:
#                                        {"counts": {"salvo": 3}, "latest": [{"g": "salvo", ...}]}
#   POST funkey-comments.rb?game=salvo   body: a name, a new line, the text;
#                                        the answer is the comments with it
#
# A comment shows at once. To take one out, delete its line in
# comments-<game>.txt in the lists folder (the one scores.rb uses). One
# address may send a comment every 30 seconds.
#
# What it touches: recent-comments.txt and one comments-<game>.txt for
# each game in DIR, nothing else. No shell, no eval, no file named by the
# request: the game is checked against GAMES. The text is kept and sent as
# text; the page shows it as text, never as HTML.

require "digest"
require "json"

DIR = ENV["FUNKEY_SCORES"] || File.expand_path("../../funkey-scores", File.dirname(File.realpath(__FILE__)))
GAMES = %w[raid kart salvo wargame stack again marble chomp eliminator vector blocks jumpman].freeze
KEEP = 300
WAIT = 30
CROWD = 1000
NAME = 30
TEXT = 1000

File.umask(0o002)

def reply(status, body, type = "application/json")
  print "Status: #{status}\r\n"
  print "Content-Type: #{type}; charset=utf-8\r\n"
  print "Access-Control-Allow-Origin: *\r\n"
  print "Cache-Control: no-store\r\n"
  print "X-Content-Type-Options: nosniff\r\n\r\n"
  print body
  exit
end

def refuse(status, why)
  reply(status, why + "\n", "text/plain")
end

def path(game)
  File.join(DIR, "comments-#{game}.txt")
end

# A kept comment: a time, a name and a text, each checked again, so a
# line spoiled by hand is left out rather than sent.
def parse(line)
  h = JSON.parse(line)
  return nil unless h.is_a?(Hash) && h["t"].is_a?(Integer) && h["n"].is_a?(String) && h["c"].is_a?(String)
  return nil if h["n"].length > NAME || h["c"].empty? || h["c"].length > TEXT
  { "t" => h["t"], "n" => h["n"], "c" => h["c"] }
rescue JSON::ParserError
  nil
end

def read(game)
  File.exist?(path(game)) ? File.readlines(path(game), encoding: "UTF-8").map { |l| parse(l) }.compact : []
end

# Text as typed, less what could fool a reader: control characters (new
# lines stay in the text) and the marks that turn writing right to left.
def clean(s, lines)
  s = s.gsub(/\r\n?/, "\n")
  s = s.gsub(/[\u202A-\u202E\u2066-\u2069\u200E\u200F]/, "")
  s = lines ? s.gsub(/[\p{Cc}&&[^\n]]/, "") : s.gsub(/\p{Cc}/, " ")
  s = s.gsub(/\n{3,}/, "\n\n") if lines
  s.strip
end

# One comment per address every WAIT seconds. Addresses are kept only as
# a short hash, and only for that long.
def allowed?
  addr = ENV["REMOTE_ADDR"].to_s
  addr = addr.split(":").first(4).join(":") if addr.include?(":")
  key = Digest::SHA256.hexdigest("funkey-comments:#{addr}")[0, 16]
  now = Time.now.to_i
  File.open(File.join(DIR, "recent-comments.txt"), File::RDWR | File::CREAT, 0o660) do |f|
    f.flock(File::LOCK_EX)
    seen = f.read.lines.map(&:split).select { |_, t| now - t.to_i < WAIT }
    return false if seen.size >= CROWD || seen.any? { |k, _| k == key }
    seen << [key, now.to_s]
    f.rewind
    f.truncate(0)
    f.write(seen.map { |k, t| "#{k} #{t}\n" }.join)
  end
  true
end

begin
  game = ENV["QUERY_STRING"].to_s[/(?:\A|&)game=([a-z]+)(?:&|\z)/, 1]
  case ENV["REQUEST_METHOD"]
  when "GET", "HEAD"
    if game == "all"
      lists = GAMES.map { |g| [g, read(g)] }
      latest = lists.flat_map { |g, l| l.last(8).map { |e| e.merge("g" => g) } }.max_by(8) { |e| e["t"] }
      reply("200 OK", JSON.generate("counts" => lists.to_h { |g, l| [g, l.size] }, "latest" => latest))
    end
    refuse("400 Bad Request", "Which game?") unless GAMES.include?(game)
    reply("200 OK", JSON.generate(read(game)))
  when "POST"
    refuse("400 Bad Request", "Which game?") unless GAMES.include?(game)
    len = ENV["CONTENT_LENGTH"].to_i
    refuse("413 Payload Too Large", "Too long.") if len > 4 * (NAME + TEXT) + 1
    refuse("400 Bad Request", "Send a name, a new line and the text.") if len < 1
    body = $stdin.read(len).to_s.force_encoding("UTF-8")
    refuse("400 Bad Request", "Send text as UTF-8.") unless body.valid_encoding?
    name, text = body.split("\n", 2)
    name = clean(name.to_s, false)
    text = clean(text.to_s, true)
    name = "A player" if name.empty?
    refuse("400 Bad Request", "A name of at most #{NAME} letters.") if name.length > NAME
    refuse("400 Bad Request", "Write something.") if text.empty?
    refuse("400 Bad Request", "At most #{TEXT} letters.") if text.length > TEXT
    refuse("429 Too Many Requests", "One comment every #{WAIT} seconds.") unless allowed?
    entry = { "t" => Time.now.to_i, "n" => name, "c" => text }
    File.open(path(game), File::RDWR | File::CREAT, 0o664) do |f|
      f.flock(File::LOCK_EX)
      list = f.read.force_encoding("UTF-8").lines.map { |l| parse(l) }.compact
      list = (list << entry).last(KEEP)
      f.rewind
      f.truncate(0)
      f.write(list.map { |e| JSON.generate(e) + "\n" }.join)
      f.flush
      reply("200 OK", JSON.generate(list))
    end
  else
    refuse("405 Method Not Allowed", "GET or POST.")
  end
rescue StandardError => e
  warn "funkey-comments: #{e.class}: #{e.message}"
  refuse("500 Internal Server Error", "Try again later.")
end
