local n = tonumber(arg[1])
local sum = 0
for i = 1, n do sum = (sum + i) % 1000000007 end
print(string.format("%.0f", sum))
