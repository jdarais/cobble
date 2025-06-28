local python = require("cobble.python")

python.python_project {
    constraints_file_calc = "/constraints_file_name",
    build_venv = "/pip_tools_venv"
}
