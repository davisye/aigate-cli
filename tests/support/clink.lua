-- Small API harness: execute the emitted Lua and inspect Clink registrations.
-- This does not simulate native cmd.exe / Readline completion or quoting.
local registrations = {}
local matcher = {}
matcher.__index = matcher
function matcher.__concat(name, parser) return { name = name, parser = parser } end
function matcher:addarg(choices)
    table.insert(self.args, choices)
    return self
end
function matcher:addflags(choices)
    for _, value in ipairs(choices) do table.insert(self.flags, value) end
    return self
end
function matcher:nofiles() self.no_files = true; return self end
clink = {}
function clink.filematches() end
function clink.argmatcher(...)
    local parser = setmetatable({ args = {}, flags = {} }, matcher)
    for _, name in ipairs({...}) do registrations[name] = parser end
    return parser
end
