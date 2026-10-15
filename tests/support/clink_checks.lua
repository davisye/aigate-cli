local root = assert(registrations['aigate-cli'])
assert(root == registrations['aigate-cli.exe'])
local function link(list, name)
    for _, choice in ipairs(list) do
        if type(choice) == 'table' and choice.name == name then return choice.parser end
    end
    error('missing link: '..name)
end
local function contains(list, expected)
    for _, value in ipairs(expected) do
        local found = false
        for _, choice in ipairs(list) do if choice == value then found = true end end
        assert(found, 'missing value: '..value)
    end
end
local workflow = link(root.args[1], 'workflow')
local comfy = link(root.args[1], 'comfy')
link(workflow.args[1], 'run')
link(comfy.args[1], 'run')
assert(link(root.args[1], 'gpus') == link(root.args[1], 'skus'))
assert(link(root.args[1], 'stock') == link(root.args[1], 'skus'))
assert(link(root.args[1], 'rm') == link(root.args[1], 'release'))
local create = link(root.args[1], 'create')
contains(link(create.flags, '--area').args[1], {'华东一区', '华东二区'})
contains(link(create.flags, '--area=').args[1], {'华东一区', '华东二区'})
contains(link(create.flags, '--image-type').args[1], {'2', '3'})
contains(link(link(root.args[1], 'list').flags, '--operation-status').args[1], {'2', '4'})
contains(link(link(workflow.args[1], 'upload').flags, '--media-type').args[1], {'image', 'video', 'audio'})
contains(link(root.args[1], 'init').args[1], {'bash', 'zsh', 'fish', 'powershell', 'clink', 'nushell'})
assert(link(link(comfy.args[1], 'run').flags, '--input').args[1][1] == clink.filematches)
assert(link(workflow.args[1], 'upload').args[1][1] == clink.filematches)
assert(root.no_files)
