-- since Lua can't access keypresses this will be AI driven

PIPE_WIDTH = 1.5
GAP_HEIGHT = 5
PIPE_SPEED = 15

GAP_MIN_Y = SCREEN_Y / 4
GAP_MAX_Y = 3 / 4 * SCREEN_Y

BIRD_JUMP_ACCEL = 20
BIRD_JUMP_COOLDOWN = 10
CURRENT_JUMP_COOLDOWN = 0
BIRD_RADIUS = 1

CURRENT_GAP_Y = 0
function reset_gap_y()
    CURRENT_GAP_Y = math.random(0, 100) / 100 * (GAP_MAX_Y - GAP_MIN_Y) + GAP_MIN_Y
end
reset_gap_y()

function calculate_top_pipe_y()
    return CURRENT_GAP_Y - (GAP_HEIGHT / 2) - (SCREEN_Y / 2)
end

function calculate_bottom_pipe_y()
    return CURRENT_GAP_Y + (GAP_HEIGHT / 2) + (SCREEN_Y / 2)
end

function bird_jump(bird)
    bird.y_vel = bird.y_vel - BIRD_JUMP_ACCEL
    CURRENT_JUMP_COOLDOWN = BIRD_JUMP_COOLDOWN
end

function bird_update(obj)
    obj.y_vel = obj.y_vel + 1

    if obj.y > CURRENT_GAP_Y + GAP_HEIGHT / 2 - (BIRD_RADIUS * 2) and CURRENT_JUMP_COOLDOWN <= 0 and obj.y_vel > -BIRD_JUMP_ACCEL / 8 then
        print("jump")
        bird_jump(obj)
    end

    CURRENT_JUMP_COOLDOWN = CURRENT_JUMP_COOLDOWN - 1

    return obj
end

GAP_RESET = false
function pipe_update(obj)
    if obj.x + PIPE_WIDTH < 0 then
        obj.x = SCREEN_X + PIPE_WIDTH
        GAP_RESET = not GAP_RESET

        if GAP_RESET then
            reset_gap_y()
        end

        if obj.name == "top_pipe" then
            obj.y = calculate_top_pipe_y()
        else
            obj.y = calculate_bottom_pipe_y()
        end
    end
    return obj
end

add_shape({shape="circle", x = SCREEN_X / 8, y = SCREEN_Y / 2, r = BIRD_RADIUS, mass = 1, update_function = "bird_update"})

-- upper pipe
add_shape({shape="rect", x=SCREEN_X + PIPE_WIDTH, y = calculate_top_pipe_y(), w = PIPE_WIDTH, h = SCREEN_Y / 2,
mass = 1, x_vel = -PIPE_SPEED, update_function="pipe_update", name="top_pipe"})

-- lower pipe
add_shape({shape="rect", x=SCREEN_X + PIPE_WIDTH, y = calculate_bottom_pipe_y(), w = PIPE_WIDTH, h = SCREEN_Y / 2,
mass = 1, x_vel = -PIPE_SPEED, update_function="pipe_update", name="bottom_pipe"})

GRAVITY = 0