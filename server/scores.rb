#!/usr/bin/env ruby
# funkey scores: one shared top ten per game, for the funkey games played
# in a web page. A CGI script; on isene.com nginx hands it the request
# through fcgiwrap, from cgi-bin/funkey-scores.rb, a link to this file in
# a checkout of the funkey repo.
#
#   GET  funkey-scores.rb?game=stack   the list
#   POST funkey-scores.rb?game=stack   body "ABC 12345 40 5": initials,
#                                      score, rows, level; the answer is
#                                      the list with it
#
# A list is a line per score, best first: "ABC 12345 40 5". The lists
# live in FUNKEY_SCORES (default ~/funkey-scores on the server, writable
# by www-data), outside the web root; edit a file there to take a score
# out. One address may send a score every 15 seconds.

require "digest"

DIR = ENV["FUNKEY_SCORES"] || "/home/geir/funkey-scores"
GAMES = %w[stack].freeze
KEEP = 10
WAIT = 15

def reply(status, body)
  print "Status: #{status}\r\n"
  print "Content-Type: text/plain; charset=utf-8\r\n"
  print "Access-Control-Allow-Origin: *\r\n"
  print "Cache-Control: no-store\r\n\r\n"
  print body
  exit
end

def parse(line)
  m = line.strip.match(/\A([A-Z]{3}) (\d{1,7}) (\d{1,5}) (\d{1,3})\z/) or return nil
  [m[1], m[2].to_i, m[3].to_i, m[4].to_i]
end

def text(list)
  list.map { |e| e.join(" ") + "\n" }.join
end

def read_list(path)
  File.exist?(path) ? File.readlines(path).map { |l| parse(l) }.compact : []
end

# A score the game could give: the level at most the highest starting level
# plus one for every ten rows, and the points far below what the best play
# earns for those rows.
def possible?(score, rows, level)
  level.between?(1, 16 + rows / 10) && score <= 3000 * level * (rows + 5) + 20_000
end

# One score per address every WAIT seconds. Addresses are kept only as a
# short hash, and only for that long.
def allowed?
  key = Digest::SHA256.hexdigest("funkey:#{ENV['REMOTE_ADDR']}")[0, 16]
  now = Time.now.to_i
  File.open(File.join(DIR, "recent.txt"), File::RDWR | File::CREAT, 0o660) do |f|
    f.flock(File::LOCK_EX)
    seen = f.read.lines.map(&:split).select { |_, t| now - t.to_i < WAIT }
    return false if seen.any? { |k, _| k == key }
    seen << [key, now.to_s]
    f.rewind
    f.truncate(0)
    f.write(seen.map { |k, t| "#{k} #{t}\n" }.join)
  end
  true
end

game = ENV["QUERY_STRING"].to_s[/(?:\A|&)game=([a-z]+)/, 1]
reply("400 Bad Request", "Which game?\n") unless GAMES.include?(game)
path = File.join(DIR, "#{game}.txt")

case ENV["REQUEST_METHOD"]
when "GET", "HEAD"
  reply("200 OK", text(read_list(path)))
when "POST"
  len = ENV["CONTENT_LENGTH"].to_i
  reply("413 Payload Too Large", "Too long.\n") if len > 64
  entry = parse($stdin.read(len).to_s)
  reply("400 Bad Request", "Send: ABC 12345 40 5\n") unless entry
  reply("400 Bad Request", "The game cannot give that score.\n") unless possible?(*entry[1..3])
  reply("429 Too Many Requests", "One score every #{WAIT} seconds.\n") unless allowed?
  File.open(path, File::RDWR | File::CREAT, 0o664) do |f|
    f.flock(File::LOCK_EX)
    list = f.read.lines.map { |l| parse(l) }.compact
    list << entry
    # Best first; of two equal scores the older stays above.
    list = list.each_with_index.sort_by { |e, i| [-e[1], i] }.map(&:first).first(KEEP)
    f.rewind
    f.truncate(0)
    f.write(text(list))
    f.flush
    reply("200 OK", text(list))
  end
else
  reply("405 Method Not Allowed", "GET or POST.\n")
end
