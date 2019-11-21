function addzhist --on-variable PWD
    zox --add "$PWD"
end

function z -d "Jump to a recent directory."
    cd (zox $argv)
end