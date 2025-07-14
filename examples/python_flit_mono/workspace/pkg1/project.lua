local python = require("python")

python.python_project {
    constraints_file_calc = "/constraints_file_name",
    build_venv = "/pip_tools_venv",
    dev_dependencies = {
        "mypy",
        "pylint",
        "pytest"
    }
}
