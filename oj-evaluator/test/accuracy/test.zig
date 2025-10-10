const std = @import("std");

pub fn main() !void {
    const stdin = std.io.getStdIn().reader();
    const stdout = std.io.getStdOut().writer();
    
    var buf: [256]u8 = undefined;
    
    while (try stdin.readUntilDelimiterOrEof(buf[0..], '\n')) |line| {
        const trimmed = std.mem.trim(u8, line, " \n\r\t");
        var iter = std.mem.splitScalar(u8, trimmed, ' ');
        const a_str = iter.next() orelse continue;
        const a = std.fmt.parseInt(i32, a_str, 10) catch continue;
        const b_str = iter.next() orelse continue;
        const b = std.fmt.parseInt(i32, b_str, 10) catch continue;
        try stdout.print("{}\n", .{a + b});
    }
}