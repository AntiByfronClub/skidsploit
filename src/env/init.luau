local http = game:GetService("HttpService")
local gui = game:GetService("CoreGui")
local packages = game:GetService("CorePackages")

local bridge = "http://127.0.0.1:6767"
local identity = "%client_pid%"

if gui:FindFirstChild(identity) then return end

local env = getfenv(0)
local rawTypeof = typeof
local genvTable = {}

local userEnv = setmetatable({}, {
    __index = function(_, key)
        local value = rawget(genvTable, key)
        if value ~= nil then
            return value
        end
        return env[key]
    end,
})

env.getgenv = setmetatable({}, {
    __call = function()
        return genvTable
    end,
    __index = genvTable,
    __newindex = genvTable,
})

env.identifyexecutor = function()
    return "skidsploit", "vskid"
end

local envFolder = Instance.new("Folder", gui)
envFolder.Name = "skid"

local function meet(root)
    for _, item in ipairs(root:GetDescendants()) do
        if item:IsA("ModuleScript") then
            return item
        end
    end
    return nil
end

local function compact(label)
    local stock = packages:FindFirstChild("Packages")
    if not stock then return nil end
    local donor = meet(stock)
    if not donor then return nil end
    local shell = donor:Clone()
    shell.Name = label
    shell.Parent = envFolder
    return shell
end

local function post(path, body, timeout)
    local verdict, servedBody = nil, ""
    local settled = false

    http:RequestInternal({
        Url = bridge .. path,
        Method = "POST",
        Body = body or "",
        Headers = { ["Content-Type"] = "text/plain" }
    }):Start(function(ok, back)
        settled = true
        if ok then
            verdict = back.StatusCode
            servedBody = back.Body or ""
        end
    end)

    local deadline = tick() + (timeout or 5)
    while not settled and tick() < deadline do task.wait() end

    return verdict, servedBody
end

local base64Alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
local base64Lookup = {}
for index = 1, #base64Alphabet do
    base64Lookup[base64Alphabet:sub(index, index)] = index - 1
end

local function base64decode(encoded)
    local chunks = {}
    local accumulator = 0
    local bits = 0
    for index = 1, #encoded do
        local value = base64Lookup[encoded:sub(index, index)]
        if value then
            accumulator = accumulator * 64 + value
            bits += 6
            if bits >= 8 then
                bits -= 8
                chunks[#chunks + 1] = string.char(math.floor(accumulator / 2 ^ bits) % 256)
                accumulator = accumulator % 2 ^ bits
            end
        end
    end
    return table.concat(chunks)
end

local httpMethods = { GET = true, POST = true, PUT = true, DELETE = true, PATCH = true }

env.request = function(options)
    assert(type(options) == "table", "invalid argument #1 to 'request' (table expected, got " .. type(options) .. ") ", 2)
    assert(type(options.Url) == "string", "invalid option 'Url' for argument #1 to 'request' (string expected, got " .. type(options.Url) .. ") ", 2)

    options.Method = type(options.Method) == "string" and options.Method:upper() or "GET"
    assert(httpMethods[options.Method], "invalid option 'Method' for argument #1 to 'request' (a valid http method expected, got '" .. options.Method .. "') ", 2)
    assert(not (options.Method == "GET" and options.Body), "invalid option 'Body' for argument #1 to 'request' (current method is GET but option 'Body' was used)", 2)

    if options.Body then
        assert(type(options.Body) == "string", "invalid option 'Body' for argument #1 to 'request' (string expected, got " .. type(options.Body) .. ") ", 2)
    else
        options.Body = ""
    end

    if options.Headers == nil then
        options.Headers = {}
    end
    assert(type(options.Headers) == "table", "invalid option 'Headers' for argument #1 to 'request' (table expected, got " .. type(options.Headers) .. ") ", 2)

    for name, value in pairs(options.Headers) do
        assert(type(name) == "string" and type(value) == "string", "invalid option 'Headers' for argument #1 to 'request' (string keys and values expected)", 2)
    end

    options.Headers["User-Agent"] = options.Headers["User-Agent"] or "skidsploit"

    local verdict, servedBody = post("/request", http:JSONEncode({
        l = options.Url,
        m = options.Method,
        h = options.Headers,
        b = options.Body
    }), 35)

    if verdict == nil or servedBody == "" then
        return {
            Success = false,
            StatusMessage = "Can't connect to skidsploit bridge!",
            StatusCode = 599,
            Body = "",
            HttpError = Enum.HttpError.ConnectFail,
            Headers = {},
            Version = "HTTP/1.1"
        }
    end

    local okDecode, result = pcall(http.JSONDecode, http, servedBody)
    if not okDecode then
        verdict = nil
    end

    if verdict == nil then
        return {
            Success = false,
            StatusMessage = "Unknown",
            StatusCode = 0,
            Body = "",
            HttpError = Enum.HttpError.ConnectFail,
            Headers = {},
            Version = "HTTP/1.1"
        }
    end

    local statusCode = tonumber(result.c) or 0
    local statusMessage = result.r
    if statusMessage ~= "OK" then
        statusMessage = "Unknown"
    end

    local httpError = nil
    pcall(function() httpError = Enum.HttpError[statusMessage] end)

    return {
        Success = statusCode >= 200 and statusCode < 300,
        StatusMessage = statusMessage,
        StatusCode = statusCode,
        Body = base64decode(tostring(result.b or "")),
        HttpError = httpError,
        Headers = type(result.h) == "table" and result.h or {},
        Version = result.v or "HTTP/1.1"
    }
end

env.http_request = env.request

local syn = {}
syn.request = env.request
env.syn = syn

env.requestAsync = function(options, callback)
    assert(type(options) == "table", "invalid argument #1 to 'requestAsync' (table expected, got " .. type(options) .. ") ", 2)
    task.spawn(function()
        local response = env.request(options)
        if type(callback) == "function" then
            callback(response)
        end
    end)
end

env.HttpGet = function(url, returnRaw)
    assert(type(url) == "string", "invalid argument #1 to 'HttpGet' (string expected, got " .. type(url) .. ") ", 2)

    local response = env.request({
        Url = url,
        Method = "GET"
    })

    if returnRaw == false then
        local okDecode, decoded = pcall(http.JSONDecode, http, tostring(response.Body))
        if okDecode then
            return decoded
        end
        return nil, "failed to decode json"
    end

    return response.Body
end

env.httpget = env.HttpGet
env.http_get = env.HttpGet
env.HttpGetAsync = env.HttpGet
env.httpGetAsync = env.HttpGet

env.HttpPost = function(url, body, contentType)
    assert(type(url) == "string", "invalid argument #1 to 'HttpPost' (string expected, got " .. type(url) .. ") ", 2)

    if body ~= nil then
        assert(type(body) == "string", "invalid argument #2 to 'HttpPost' (string expected, got " .. type(body) .. ") ", 2)
    else
        body = ""
    end

    return env.request({
        Url = url,
        Method = "POST",
        Body = body,
        Headers = {
            ["Content-Type"] = type(contentType) == "string" and contentType or "application/json"
        }
    })
end

env.httppost = env.HttpPost
env.http_post = env.HttpPost
env.HttpPostAsync = env.HttpPost
env.httpPostAsync = env.HttpPost

local proxiedData = {}

local namecallCache = {}

local function generateProbe(object)
    local _, err = xpcall(function()
        object:__namecall()
    end, function()
        return debug.info(2, "f")
    end)
    return err
end

local function extractMethod(err)
    if type(err) ~= "string" then
        return nil
    end
    return err:match("^(.+) is not a valid member of %w+$")
end

local firstProbe = generateProbe(OverlapParams.new())
local secondProbe = generateProbe(Color3.new())

env.getnamecallmethod = function()
    local _, err = pcall(firstProbe)
    local method = extractMethod(err)
    if not method then
        _, err = pcall(secondProbe)
        method = extractMethod(err)
    end

    local fixerData = newproxy(true)
    local fixerMeta = getmetatable(fixerData)
    fixerMeta.__namecall = function()
        local _, probeError = pcall(firstProbe)
        local probeMethod = extractMethod(probeError)
        if not probeMethod then
            _, probeError = pcall(secondProbe)
            probeMethod = extractMethod(probeError)
        end
    end
    fixerData:__namecall()

    if not method or method == "__namecall" then
        local cached = namecallCache[coroutine.running()]
        if cached then
            return cached
        end
        return nil
    end

    namecallCache[coroutine.running()] = method
    return method
end

local function proxyIndex(t, name)
    local data = proxiedData[t]
    local override = data.methods[name]
    if override then
        return function(_, ...)
            return override(...)
        end
    end

    local value = data.target[name]
    if type(value) == "function" then
        return function(_, ...)
            return value(data.target, ...)
        end
    end
    return value
end

local function proxyNamecall(t, ...)
    local data = proxiedData[t]
    local method = env.getnamecallmethod()

    if method and data.methods[method] then
        return data.methods[method](...)
    end

    if method then
        return data.target[method](data.target, ...)
    end

    return nil
end

local function proxyNewIndex(t, name, value)
    local data = proxiedData[t]
    data.target[name] = value
end

local function proxyToString(t)
    return proxiedData[t].target.Name
end

local function proxyObject(target, methods)
    if proxiedData[target] then
        return proxiedData[target].proxy
    end

    local proxy = newproxy(true)
    local proxyMeta = getmetatable(proxy)
    proxyMeta.__index = proxyIndex
    proxyMeta.__namecall = proxyNamecall
    proxyMeta.__newindex = proxyNewIndex
    proxyMeta.__tostring = proxyToString
    proxyMeta.__metatable = getmetatable(target)

    local data = {}
    data.target = target
    data.proxy = proxy
    data.methods = methods or {}

    proxiedData[target] = data
    proxiedData[proxy] = data
    return proxy
end

env.typeof = function(value)
    local base = rawTypeof(value)
    if base == "userdata" and proxiedData[value] then
        return "Instance"
    end
    return base
end

local proxiedGame = proxyObject(game, {
    HttpGet = env.HttpGet,
    HttpGetAsync = env.HttpGet,
    HttpPost = env.HttpPost,
    HttpPostAsync = env.HttpPost
})

env.game = proxiedGame
env.Game = proxiedGame

env.loadstring = function(code, chunkname)
    assert(type(code) == "string", "invalid argument #1 to 'loadstring' (string expected, got " .. type(code) .. ") ", 2)

    chunkname = type(chunkname) == "string" and chunkname or "loadstring"
    chunkname = chunkname:gsub("[^%a_]", "")

    if code == "" then
        return nil, "Empty script source"
    end

    local source = "return{[ [[" .. chunkname .. "]] ]=function(...)" .. code .. "\nend}"

    local tag = tostring(math.random(1e6, 1e9))
    local shell = compact(tag)
    if not shell then
        return nil, "No module script available"
    end

    local verdict, servedBody = post("/loadstring?c=" .. tag .. "&p=" .. identity, source)

    if verdict ~= 200 then
        shell:Destroy()
        if verdict == nil then
            return nil, "Bridge not responding"
        end
        if servedBody == "" or servedBody == "attach first" then
            return nil, "Failed to compile"
        end
        return nil, servedBody
    end

    local loaded, loader = pcall(require, shell)
    shell:Destroy()

    if not loaded then
        return nil, tostring(loader)
    end
    if type(loader) ~= "function" then
        return nil, "Failed to load"
    end

    local ran, built = pcall(loader)
    if not ran or type(built) ~= "table" then
        return nil, "Failed to load"
    end

    local func = built[chunkname]
    if type(func) ~= "function" then
        return nil, "Failed to load"
    end

    pcall(setfenv, func, userEnv)
    return func
end

task.spawn(function()
    while task.wait() do
        local tag = tostring(math.random(1e6, 1e9))
        local shell = compact(tag)
        if not shell then
            task.wait(1)
        else
            local verdict, servedBody = post("/ls?c=" .. tag .. "&p=" .. identity)

            if verdict == 200 then
                local good, run = pcall(require, shell)
                if good and type(run) == "function" then
                    env.script = shell
                    pcall(setfenv, run, userEnv)

                    task.spawn(function()
                        local ok, err = pcall(run)
                        if not ok then
                            warn("skidsploit -- " .. tostring(err))
                        end
                    end)

                    env.script = nil
                end
            elseif verdict ~= nil and verdict >= 400 and servedBody ~= "" and servedBody ~= "attach first" then
                warn("skidsploit -- " .. servedBody)
            end

            shell:Destroy()

            if verdict ~= 200 then
                task.wait()
            end
        end
    end
end)

return setmetatable({}, {
    __index = function()
        return function() end
    end
})
