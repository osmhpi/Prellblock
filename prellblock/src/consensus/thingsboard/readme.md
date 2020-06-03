run ```docker-compose up```

After executing this command you can open http://{your-host-ip}:8080 in you browser (for ex. http://localhost:8080). You should see ThingsBoard login page. Use the following default credentials:

    Systen Administrator: sysadmin@thingsboard.org / sysadmin
    Tenant Administrator: tenant@thingsboard.org / tenant
    Customer User: customer@thingsboard.org / customer

Use the **tenant** account to create an manage devices.

You can send value to a device using the following command:
```sh
curl -v -X POST -d "{\"temperature\": 25}" http://localhost:8080/api/v1/$ACCESS_TOKEN/telemetry --header "Content-Type:application/json"
```
