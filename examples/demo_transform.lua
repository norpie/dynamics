-- Demo Lua Transform Script
-- Demonstrates: fetch, delete, update, create operations with status logging

local M = {}

-- Declare function - returns what we need from source and target
function M.declare()
    return {
        source = {
            contact = {
                fields = { "contactid", "firstname", "lastname", "emailaddress1" },
                filter = "firstname ne null"
            }
        },
        target = {
            contact = {
                fields = { "contactid", "firstname", "lastname", "emailaddress1" }
            }
        }
    }
end

-- Transform function - called by the runtime with fetched data
function M.transform(source, target)
    lib.status("Starting contact transformation...")
    
    local contacts = source.contact or {}
    lib.log("Processing " .. #contacts .. " contacts")
    
    local ops = {}
    local deleted = 0
    local updated = 0
    
    for i, contact in ipairs(contacts) do
        local firstname = contact.firstname or ""
        
        -- Delete contacts starting with 'A'
        if firstname:sub(1, 1):upper() == "A" then
            lib.status("Deleting contact: " .. firstname)
            table.insert(ops, {
                entity = "contact",
                operation = "delete",
                id = contact.contactid
            })
            deleted = deleted + 1
            
        -- Update contacts starting with 'B' -> change to 'C'
        elseif firstname:sub(1, 1):upper() == "B" then
            local new_firstname = "C" .. firstname:sub(2)
            lib.status("Updating contact: " .. firstname .. " -> " .. new_firstname)
            table.insert(ops, {
                entity = "contact",
                operation = "update",
                id = contact.contactid,
                fields = {
                    firstname = new_firstname
                }
            })
            updated = updated + 1
        end
        
        -- Report progress every 10 contacts
        if i % 10 == 0 then
            lib.progress(i, #contacts)
        end
    end
    
    -- Create a new test contact
    lib.status("Creating new test contact...")
    table.insert(ops, {
        entity = "contact",
        operation = "create",
        fields = {
            firstname = "LuaTest",
            lastname = "Generated",
            emailaddress1 = "luatest@example.com"
        }
    })
    
    -- Final summary
    lib.status("Transformation complete!")
    lib.log("Summary: Deleted " .. deleted .. ", Updated " .. updated .. ", Created 1")
    
    return ops
end

return M
