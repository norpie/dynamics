# Lua Transform Quick Reference

Fast reference for writing Dynamics 365 Lua transform scripts.

---

## Script Template

```lua
local M = {}

function M.declare()
    return {
        source = {
            entity_name = {
                fields = { "field1", "field2" },
                filter = "statecode eq 0",  -- Optional
                expand = { "lookup" },      -- Optional
                top = 1000                  -- Optional
            }
        },
        target = {
            entity_name = {
                fields = { "field1", "field2" }
            }
        }
    }
end

function M.transform(source, target)
    local ops = {}
    
    -- Your logic here
    
    return ops
end

return M
```

---

## Operations

### Create
```lua
{
    entity = "account",
    operation = "create",
    fields = {
        name = "Acme Corp",
        revenue = 1000000
    }
}
```

### Update
```lua
{
    entity = "account",
    operation = "update",
    id = "guid-here",
    fields = {
        revenue = 2000000
    }
}
```

### Delete
```lua
{
    entity = "contact",
    operation = "delete",
    id = "guid-here"
}
```

### Deactivate
```lua
{
    entity = "account",
    operation = "deactivate",
    id = "guid-here"
}
```

### Skip (audit)
```lua
{
    entity = "contact",
    operation = "skip",
    id = "guid-here",
    reason = "Missing required field"
}
```

### Error (audit)
```lua
{
    entity = "account",
    operation = "error",
    id = "guid-here",
    error = "Invalid data"
}
```

---

## Standard Library

### Collections
```lua
lib.find(records, "fieldname", value)           -- Find first match
lib.filter(records, function(r) return test end) -- Filter by predicate
lib.map(records, function(r) return new_value end) -- Transform records
lib.group_by(records, "fieldname")              -- Group by field value
```

### GUIDs
```lua
lib.guid()           -- Generate new GUID
lib.is_guid(value)   -- Check if valid GUID
```

### Strings
```lua
lib.lower(s)              -- To lowercase
lib.upper(s)              -- To uppercase
lib.trim(s)               -- Remove whitespace
lib.split(s, delim)       -- Split by delimiter
lib.contains(s, sub)      -- Check substring
lib.starts_with(s, pre)   -- Check prefix
lib.ends_with(s, suf)     -- Check suffix
```

### Dates
```lua
lib.now()                 -- Current ISO datetime
lib.parse_date(s)         -- Parse to ISO format
lib.format_date(dt, fmt)  -- Format datetime
```

### Types
```lua
lib.is_nil(v)
lib.is_string(v)
lib.is_number(v)
lib.is_table(v)
lib.is_boolean(v)
```

### Logging & Progress
```lua
lib.log("Info message")
lib.warn("Warning message")
lib.status("Current status")
lib.progress(current, total)
```

---

## Common Patterns

### Match Source to Target
```lua
local source_records = source.account or {}
local target_records = target.account or {}

for _, src in ipairs(source_records) do
    local tgt = lib.find(target_records, "accountid", src.accountid)
    
    if tgt then
        -- Update existing
        table.insert(ops, {
            entity = "account",
            operation = "update",
            id = tgt.accountid,
            fields = { name = src.name }
        })
    else
        -- Create new
        table.insert(ops, {
            entity = "account",
            operation = "create",
            fields = { name = src.name }
        })
    end
end
```

### Build Lookup Map
```lua
-- Build map: name -> id
local target_map = {}
for _, acc in ipairs(target.account or {}) do
    target_map[lib.lower(acc.name or "")] = acc.accountid
end

-- Use map
for _, contact in ipairs(source.contact or {}) do
    local account_id = target_map[lib.lower(contact.company or "")]
    if account_id then
        -- Link to account
    end
end
```

### Conditional Logic
```lua
for _, record in ipairs(source.contact or {}) do
    local name = record.firstname or ""
    
    if lib.starts_with(name, "A") then
        -- Delete
        table.insert(ops, {
            entity = "contact",
            operation = "delete",
            id = record.contactid
        })
    elseif name == "Test" then
        -- Skip
        table.insert(ops, {
            entity = "contact",
            operation = "skip",
            id = record.contactid,
            reason = "Test contact"
        })
    else
        -- Create
        table.insert(ops, {
            entity = "contact",
            operation = "create",
            fields = { firstname = name }
        })
    end
end
```

### Progress Reporting
```lua
local records = source.account or {}

for i, record in ipairs(records) do
    -- Process record...
    
    if i % 100 == 0 then
        lib.status("Processing accounts...")
        lib.progress(i, #records)
    end
end
```

### Safe Field Access
```lua
-- Always use 'or' for nil safety
local email = contact.emailaddress1 or ""
local revenue = account.revenue or 0
local parent_id = contact["_parentcustomerid_value"] or nil

-- Check explicitly
if lib.is_nil(contact.emailaddress1) then
    lib.warn("No email for contact: " .. (contact.contactid or "unknown"))
end
```

---

## OData Filters

```lua
"statecode eq 0"                              -- Equals
"statecode ne 1"                              -- Not equals
"revenue gt 1000000"                          -- Greater than
"revenue ge 1000000"                          -- Greater or equal
"revenue lt 1000000"                          -- Less than
"revenue le 1000000"                          -- Less or equal
"statecode eq 0 and revenue gt 0"             -- AND
"statecode eq 0 or statecode eq 1"            -- OR
"not (statecode eq 2)"                        -- NOT
"contains(name, 'Corp')"                      -- Contains
"startswith(name, 'A')"                       -- Starts with
"endswith(emailaddress1, '@example.com')"     -- Ends with
```

---

## Lookup Fields

```lua
-- In declarations, use OData format:
fields = { "contactid", "_parentcustomerid_value" }

-- In operations, use logical name:
fields = {
    firstname = "John",
    parentcustomerid = "account-guid-here"  -- Not _value suffix
}
```

---

## Debugging Tips

### Enable Verbose Logging
```lua
function M.transform(source, target)
    lib.log("Source entities: " .. tostring(source))
    lib.log("Target entities: " .. tostring(target))
    
    local accounts = source.account or {}
    lib.log("Account count: " .. #accounts)
    
    if #accounts > 0 then
        local first = accounts[1]
        for k, v in pairs(first) do
            lib.log("  " .. k .. " = " .. tostring(v))
        end
    end
    
    return {}
end
```

### Test with Small Datasets
```lua
function M.declare()
    return {
        source = {
            account = {
                fields = { "accountid", "name" },
                top = 5  -- Only 5 records for testing
            }
        },
        target = {}
    }
end
```

### Check Log File
```bash
tail -f dynamics-cli.log
```

---

## Common Errors

| Error | Fix |
|-------|-----|
| `Script must return a module table` | Add `return M` at end |
| `Script must have a 'declare' function` | Check spelling: `M.declare` |
| `Operation requires id` | Add `id = "guid"` to update/delete/deactivate |
| `Create operation requires fields` | Add `fields = { ... }` with at least one field |
| `attempt to index a nil value` | Use `or {}` and `or ""` for nil safety |

---

## Limitations

**Sandboxed Environment** - Not available:
- `io` (file operations)
- `os` (system commands)
- `debug`
- `require` / `package`

**Available:**
- `table` (table functions)
- `string` (string functions)
- `math` (math functions)
- `lib.*` (custom standard library)

---

## Performance Tips

1. **Filter at source**: Use OData filters instead of Lua filtering
2. **Build lookup maps**: Don't search repeatedly
3. **Batch progress updates**: Update every 100 records, not every record
4. **Use `top` during development**: Limit records while testing

---

## When to Use Lua vs Declarative Mode

**Use Lua when:**
- Complex conditional logic needed
- Aggregating/flattening data
- Cross-entity matching/lookups
- Delete/deactivate operations required
- Business rules can't be expressed declaratively

**Use Declarative when:**
- Simple field-to-field mappings
- Straightforward lookups with resolvers
- Standard CRUD operations only

---

For detailed examples and explanations, see [LUA_TRANSFORM_GUIDE.md](./LUA_TRANSFORM_GUIDE.md)
